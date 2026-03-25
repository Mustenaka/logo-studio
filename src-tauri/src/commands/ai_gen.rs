/// Tauri commands for AI logo generation.
///
/// Commands:
///   ai_gen_device_info    — current device (CPU/GPU) info
///   ai_gen_list_models    — model catalog with download status
///   ai_gen_download       — download a model (progress via events)
///   ai_gen_generate       — run SD pipeline → base64 PNG
///   ai_gen_get_hf_token   — read stored HF access token (masked)
///   ai_gen_set_hf_token   — save HF access token to disk
///   ai_gen_delete_hf_token — remove stored token

use base64::{engine::general_purpose, Engine as _};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::ai_gen::{
    device::{detect_device, DeviceInfo},
    downloader::{
        delete_hf_token, download_model, get_paths, is_downloaded, load_hf_token,
        save_hf_token, DownloadError,
    },
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
    pub requires_token: bool,
    pub is_downloaded: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    pub success: bool,
    pub image: Option<String>,
    pub error: Option<String>,
    pub model_id: String,
    pub device_kind: String,
    pub steps_taken: u32,
}

/// Result returned by `ai_gen_download`.
///
/// The frontend uses `errorKind` to decide whether to show the token UI:
///   "auth_required" → show the HF token settings panel
///   "not_found"     → show a path/repo error
///   "error"         → generic error toast
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub success: bool,
    pub error_kind: Option<String>,   // "auth_required" | "not_found" | "error"
    pub error_message: Option<String>,
    pub error_url: Option<String>,
}

/// HF token info returned to the frontend (never exposes the raw token).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HfTokenStatus {
    /// True when a token is stored or available via env var.
    pub has_token: bool,
    /// Masked preview so the user can confirm which token is set.
    /// Format: "hf_••••••••••••••••XXXX" (last 4 chars visible)
    pub masked: Option<String>,
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn ai_gen_device_info() -> DeviceInfo {
    let (_, info) = detect_device();
    info
}

#[tauri::command]
pub fn ai_gen_list_models() -> Vec<ModelStatus> {
    catalog().iter().map(model_to_status).collect()
}

/// Download all weight files for `model_id`.
///
/// Returns a `DownloadResult` instead of `Result<(), String>` so the frontend
/// can distinguish auth errors from other failures without parsing strings.
#[tauri::command]
pub async fn ai_gen_download(
    app: AppHandle,
    model_id: String,
) -> DownloadResult {
    let def = match find(&model_id) {
        Some(d) => d,
        None => return DownloadResult {
            success: false,
            error_kind: Some("error".into()),
            error_message: Some(format!("Unknown model id: {model_id}")),
            error_url: None,
        },
    };

    match download_model(&app, def).await {
        Ok(_) => DownloadResult {
            success: true,
            error_kind: None,
            error_message: None,
            error_url: None,
        },
        Err(DownloadError::AuthRequired { url, .. }) => DownloadResult {
            success: false,
            error_kind: Some("auth_required".into()),
            error_message: Some("需要 HuggingFace Access Token".into()),
            error_url: Some(url),
        },
        Err(DownloadError::NotFound { url }) => DownloadResult {
            success: false,
            error_kind: Some("not_found".into()),
            error_message: Some(format!("文件不存在 (404): {url}")),
            error_url: Some(url),
        },
        Err(DownloadError::Other(msg)) => DownloadResult {
            success: false,
            error_kind: Some("error".into()),
            error_message: Some(msg),
            error_url: None,
        },
    }
}

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
    match run_generate(app, model_id.clone(), prompt, negative_prompt, steps, guidance, width, height, seed).await {
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

/// Return whether a HF token is stored and a masked preview of it.
#[tauri::command]
pub fn ai_gen_get_hf_token() -> HfTokenStatus {
    match load_hf_token() {
        None => HfTokenStatus { has_token: false, masked: None },
        Some(token) => {
            // Show prefix + last 4 chars: "hf_••••••••••ABCD"
            let suffix: String = token.chars().rev().take(4).collect::<String>()
                .chars().rev().collect();
            let stars = "•".repeat(token.len().saturating_sub(4).min(16));
            let prefix = if token.starts_with("hf_") { "hf_" } else { "" };
            HfTokenStatus {
                has_token: true,
                masked: Some(format!("{prefix}{stars}{suffix}")),
            }
        }
    }
}

/// Save a HF access token to disk (persists across restarts).
#[tauri::command]
pub fn ai_gen_set_hf_token(token: String) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("Token cannot be empty".into());
    }
    save_hf_token(token.trim())
}

/// Remove the stored HF access token.
#[tauri::command]
pub fn ai_gen_delete_hf_token() -> Result<(), String> {
    delete_hf_token()
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
        .ok_or_else(|| format!("模型 '{model_id}' 尚未下载"))?;

    let (device, dev_info) = detect_device();
    let device_kind = dev_info.kind.clone();

    let n_steps = steps.unwrap_or(if dev_info.is_accelerated {
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

    let full_prompt = match def.lora.as_ref().and_then(|l| l.trigger_word) {
        Some(trigger) if !prompt.contains(trigger) => format!("{trigger}, {prompt}"),
        _ => prompt,
    };

    let neg = negative_prompt.unwrap_or_else(|| {
        "blurry, low quality, watermark, signature, text, ugly, deformed".into()
    });

    let lora = def.lora.clone();
    let model_base = def.base.clone();
    let app_clone = app.clone();
    let mid = model_id.clone();

    let png_bytes = tokio::task::spawn_blocking(move || {
        run_pipeline(&paths, &crate::ai_gen::pipeline::GenerateParams {
            model_base,
            prompt: full_prompt,
            negative_prompt: neg,
            steps: n_steps,
            guidance_scale,
            width: img_w,
            height: img_h,
            seed,
            lora,
        }, &device, |step, total| {
            let _ = app_clone.emit("ai-gen:step-progress", serde_json::json!({
                "modelId": mid,
                "step": step,
                "totalSteps": total,
            }));
        })
    })
    .await
    .map_err(|e| format!("推理线程错误: {e}"))??;

    Ok((general_purpose::STANDARD.encode(&png_bytes), device_kind, n_steps as u32))
}

fn model_to_status(def: &ModelDef) -> ModelStatus {
    use crate::ai_gen::model_registry::ModelBase;
    ModelStatus {
        id: def.id,
        name: def.name,
        description: def.description,
        base: match def.base { ModelBase::Sd15 => "sd15".into(), ModelBase::SdXl => "sdxl".into() },
        size_mb: def.size_mb,
        min_ram_mb: def.min_ram_mb,
        default_steps_cpu: def.default_steps_cpu,
        default_steps_gpu: def.default_steps_gpu,
        default_guidance: def.default_guidance,
        max_resolution: def.max_resolution,
        example_prompt: def.example_prompt,
        has_lora: def.lora.is_some(),
        lora_trigger_word: def.lora.as_ref().and_then(|l| l.trigger_word),
        requires_token: def.requires_token
            || def.lora.as_ref().map_or(false, |l| l.requires_token),
        is_downloaded: is_downloaded(def),
    }
}
