/// Tauri commands for AI logo generation.
///
/// Commands exposed to the frontend:
///   ai_gen_device_info   — returns current device (CPU/GPU) info
///   ai_gen_list_models   — lists the built-in model catalog with download status
///   ai_gen_download      — downloads a model (streams progress via events)
///   ai_gen_generate      — runs the SD pipeline and returns a base64 PNG

use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::ai_gen::{
    device::{detect_device, DeviceInfo},
    downloader::{download_model, get_paths, is_downloaded},
    model_registry::{catalog, find, ModelDef},
    pipeline::{run_pipeline, GenerateParams},
};

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub base: String,
    pub size_mb: u32,
    pub min_ram_mb: u32,
    pub default_steps_cpu: u32,
    pub default_steps_gpu: u32,
    pub default_guidance: f32,
    pub max_resolution: u32,
    pub example_prompt: &'static str,
    pub has_lora: bool,
    pub lora_trigger_word: Option<&'static str>,
    /// True when all weight files are present in the models directory
    pub is_downloaded: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    pub success: bool,
    /// base64-encoded PNG (no data-URL prefix)
    pub image: Option<String>,
    pub error: Option<String>,
    pub model_id: String,
    pub device_kind: String,
    pub steps_taken: u32,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Return information about the device that will be used for inference.
#[tauri::command]
pub fn ai_gen_device_info() -> DeviceInfo {
    let (_, info) = detect_device();
    info
}

/// Return the full model catalog with per-model download status.
#[tauri::command]
pub fn ai_gen_list_models() -> Vec<ModelStatus> {
    catalog()
        .iter()
        .map(|def| model_to_status(def))
        .collect()
}

/// Download all weight files for `model_id`.
///
/// Emits `"ai-gen:download-progress"` events during download.
/// Safe to call when model is already downloaded — files that exist are skipped.
#[tauri::command]
pub async fn ai_gen_download(
    app: AppHandle,
    model_id: String,
) -> Result<(), String> {
    let def = find(&model_id)
        .ok_or_else(|| format!("Unknown model id: {model_id}"))?;

    download_model(&app, def).await?;
    Ok(())
}

/// Generate a logo PNG via Stable Diffusion.
///
/// Returns a `GenerateResult` where `image` is a base64-encoded PNG (not a
/// data-URL — the frontend should prepend `data:image/png;base64,` itself).
///
/// Step progress is streamed via `"ai-gen:step-progress"` events:
///   `{ modelId, step, totalSteps }`
///
/// Inference runs on a blocking thread so the Tauri event loop is not blocked.
#[tauri::command]
pub async fn ai_gen_generate(
    app: AppHandle,
    model_id: String,
    prompt: String,
    negative_prompt: Option<String>,
    steps: Option<u32>,
    guidance: Option<f32>,
    width: Option<u32>,
    height: Option<u32>,
    seed: Option<u64>,
) -> GenerateResult {
    match run_generate(
        app,
        model_id.clone(),
        prompt,
        negative_prompt,
        steps,
        guidance,
        width,
        height,
        seed,
    )
    .await
    {
        Ok((image_b64, device_kind, steps_taken)) => GenerateResult {
            success: true,
            image: Some(image_b64),
            error: None,
            model_id,
            device_kind,
            steps_taken,
        },
        Err(e) => GenerateResult {
            success: false,
            image: None,
            error: Some(e),
            model_id,
            device_kind: "unknown".into(),
            steps_taken: 0,
        },
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn run_generate(
    app: AppHandle,
    model_id: String,
    prompt: String,
    negative_prompt: Option<String>,
    steps: Option<u32>,
    guidance: Option<f32>,
    width: Option<u32>,
    height: Option<u32>,
    seed: Option<u64>,
) -> Result<(String, String, u32), String> {
    let def = find(&model_id)
        .ok_or_else(|| format!("Unknown model id: {model_id}"))?;

    let paths = get_paths(def)
        .ok_or_else(|| format!("Model '{model_id}' is not downloaded yet"))?;

    let (device, dev_info) = detect_device();
    let device_kind = dev_info.kind.clone();

    // Resolve inference parameters with sensible defaults
    let n_steps = steps
        .unwrap_or(if dev_info.is_accelerated {
            def.default_steps_gpu
        } else {
            def.default_steps_cpu
        }) as usize;

    let guidance_scale = guidance.unwrap_or(def.default_guidance) as f64;

    let img_w = (width.unwrap_or(512) as usize)
        .min(def.max_resolution as usize)
        .next_multiple_of(8)
        .max(8);
    let img_h = (height.unwrap_or(512) as usize)
        .min(def.max_resolution as usize)
        .next_multiple_of(8)
        .max(8);

    // Prepend trigger word if defined
    let full_prompt = match def.lora.as_ref().and_then(|l| l.trigger_word) {
        Some(trigger) if !prompt.contains(trigger) => {
            format!("{trigger}, {prompt}")
        }
        _ => prompt,
    };

    let neg = negative_prompt.unwrap_or_else(|| {
        "blurry, low quality, watermark, signature, text, ugly, deformed".into()
    });

    let lora = def.lora.clone();
    let model_base = def.base.clone();
    let app_clone = app.clone();
    let mid = model_id.clone();

    // Run inference on a blocking thread — SD is CPU-intensive
    let png_bytes = tokio::task::spawn_blocking(move || {
        let params = GenerateParams {
            model_base,
            prompt: full_prompt,
            negative_prompt: neg,
            steps: n_steps,
            guidance_scale,
            width: img_w,
            height: img_h,
            seed,
            lora,
        };

        run_pipeline(&paths, &params, &device, |step, total| {
            let _ = app_clone.emit(
                "ai-gen:step-progress",
                serde_json::json!({
                    "modelId": mid,
                    "step": step,
                    "totalSteps": total,
                }),
            );
        })
    })
    .await
    .map_err(|e| format!("Inference thread panic: {e}"))??;

    let b64 = general_purpose::STANDARD.encode(&png_bytes);
    Ok((b64, device_kind, n_steps as u32))
}

fn model_to_status(def: &ModelDef) -> ModelStatus {
    use crate::ai_gen::model_registry::ModelBase;
    ModelStatus {
        id: def.id,
        name: def.name,
        description: def.description,
        base: match def.base {
            ModelBase::Sd15 => "sd15".into(),
            ModelBase::SdXl => "sdxl".into(),
        },
        size_mb: def.size_mb,
        min_ram_mb: def.min_ram_mb,
        default_steps_cpu: def.default_steps_cpu,
        default_steps_gpu: def.default_steps_gpu,
        default_guidance: def.default_guidance,
        max_resolution: def.max_resolution,
        example_prompt: def.example_prompt,
        has_lora: def.lora.is_some(),
        lora_trigger_word: def.lora.as_ref().and_then(|l| l.trigger_word),
        is_downloaded: is_downloaded(def),
    }
}
