/// Model downloader — fetches weight files from HuggingFace and emits
/// per-file progress events so the frontend can render a download bar.
///
/// Layout on disk:
///   <models_root>/<model_id>/
///       tokenizer/tokenizer.json
///       text_encoder/model.safetensors
///       unet/diffusion_pytorch_model.safetensors
///       vae/diffusion_pytorch_model.safetensors
///       lora.safetensors          (only if the model has a LoRA)
///
/// Environment override: set AI_GEN_MODELS_DIR to use a custom root.

use crate::ai_gen::model_registry::ModelDef;
use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

// ── Paths ─────────────────────────────────────────────────────────────────────

/// All local file paths needed to run the SD pipeline for one model.
#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub tokenizer: PathBuf,
    pub clip_weights: PathBuf,
    pub unet_weights: PathBuf,
    pub vae_weights: PathBuf,
    /// Present only when the model definition includes a LoRA.
    pub lora_weights: Option<PathBuf>,
}

/// Root directory for all AI-gen model weights.
pub fn models_root() -> PathBuf {
    if let Ok(dir) = std::env::var("AI_GEN_MODELS_DIR") {
        return PathBuf::from(dir);
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // Dev (cargo run): exe lives in target/debug/ — go two levels up to src-tauri/
    let dev_candidate = exe_dir.join("../../ai_models");
    if exe_dir
        .components()
        .any(|c| c.as_os_str() == "debug" || c.as_os_str() == "release")
    {
        return dev_candidate
            .canonicalize()
            .unwrap_or(dev_candidate);
    }
    // Production: next to the executable
    exe_dir.join("ai_models")
}

/// Per-model subdirectory.
pub fn model_dir(model_id: &str) -> PathBuf {
    models_root().join(model_id)
}

/// Build the expected on-disk paths for a given model.
fn expected_paths(def: &ModelDef, dir: &Path) -> ModelPaths {
    ModelPaths {
        tokenizer:    dir.join("tokenizer/tokenizer.json"),
        clip_weights: dir.join("text_encoder/model.safetensors"),
        unet_weights: dir.join("unet/diffusion_pytorch_model.safetensors"),
        vae_weights:  dir.join("vae/diffusion_pytorch_model.safetensors"),
        lora_weights: def
            .lora
            .as_ref()
            .map(|_| dir.join("lora.safetensors")),
    }
}

/// True when every required file is already present on disk.
pub fn is_downloaded(def: &ModelDef) -> bool {
    let dir = model_dir(def.id);
    let p = expected_paths(def, &dir);
    p.tokenizer.exists()
        && p.clip_weights.exists()
        && p.unet_weights.exists()
        && p.vae_weights.exists()
        && def
            .lora
            .as_ref()
            .map_or(true, |_| p.lora_weights.as_ref().map_or(false, |l| l.exists()))
}

/// Return local paths when the model is already fully downloaded.
pub fn get_paths(def: &ModelDef) -> Option<ModelPaths> {
    if !is_downloaded(def) {
        return None;
    }
    Some(expected_paths(def, &model_dir(def.id)))
}

// ── Progress event ────────────────────────────────────────────────────────────

/// Tauri event emitted for every downloaded chunk.
/// Frontend listens on "ai-gen:download-progress".
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_id: String,
    pub file_name: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub percent: f32,
}

// ── Download ──────────────────────────────────────────────────────────────────

/// Download all model files for `def`.
///
/// Files already present are skipped. Each download emits
/// `"ai-gen:download-progress"` events so the UI can show a progress bar.
/// Returns `ModelPaths` pointing to the locally cached files on success.
pub async fn download_model(
    app: &AppHandle,
    def: &ModelDef,
) -> Result<ModelPaths, String> {
    let dir = model_dir(def.id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Create model dir: {e}"))?;

    // (hf_repo, path_within_repo, local_destination)
    let mut files: Vec<(&str, &str, PathBuf)> = vec![
        (def.hf_repo, "tokenizer/tokenizer.json",                      dir.join("tokenizer/tokenizer.json")),
        (def.hf_repo, "text_encoder/model.safetensors",                dir.join("text_encoder/model.safetensors")),
        (def.hf_repo, "unet/diffusion_pytorch_model.safetensors",      dir.join("unet/diffusion_pytorch_model.safetensors")),
        (def.hf_repo, "vae/diffusion_pytorch_model.safetensors",       dir.join("vae/diffusion_pytorch_model.safetensors")),
    ];

    if let Some(lora) = &def.lora {
        files.push((lora.hf_repo, lora.filename, dir.join("lora.safetensors")));
    }

    let client = reqwest::Client::builder()
        .user_agent("logo-studio/1.0 (https://github.com/your-org/logo-studio)")
        .timeout(std::time::Duration::from_secs(3600))
        .build()
        .map_err(|e| format!("Build HTTP client: {e}"))?;

    for (repo, hf_path, local) in &files {
        if local.exists() {
            eprintln!("[AI-GEN] Skip (exists): {}", local.display());
            continue;
        }

        // Ensure parent directory exists
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Create dir {}: {e}", parent.display()))?;
        }

        let file_label = local
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(hf_path)
            .to_string();

        // HuggingFace CDN URL
        let url = hf_url(repo, hf_path);
        eprintln!("[AI-GEN] Downloading {file_label} from {url}");

        download_with_progress(&client, &url, local, app, def.id, &file_label).await?;
    }

    get_paths(def).ok_or_else(|| "Model files missing after download".to_string())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn hf_url(repo: &str, path: &str) -> String {
    // Respect a mirror override for restricted networks (e.g. China).
    let base = std::env::var("HF_ENDPOINT")
        .unwrap_or_else(|_| "https://huggingface.co".into());
    format!("{base}/{repo}/resolve/main/{path}")
}

async fn download_with_progress(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    app: &AppHandle,
    model_id: &str,
    file_name: &str,
) -> Result<(), String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {} downloading {url}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    // Write to a .tmp file and atomically rename on completion.
    let tmp = dest.with_extension("safetensors.tmp");
    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| format!("Create {}: {e}", tmp.display()))?;

    // Emit an initial 0% event so the UI shows the file immediately.
    let _ = app.emit(
        "ai-gen:download-progress",
        DownloadProgress {
            model_id: model_id.into(),
            file_name: file_name.into(),
            bytes_done: 0,
            bytes_total: total,
            percent: 0.0,
        },
    );

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Stream read: {e}"))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Write chunk: {e}"))?;
        downloaded += chunk.len() as u64;

        let percent = if total > 0 {
            downloaded as f32 / total as f32 * 100.0
        } else {
            0.0
        };

        let _ = app.emit(
            "ai-gen:download-progress",
            DownloadProgress {
                model_id: model_id.into(),
                file_name: file_name.into(),
                bytes_done: downloaded,
                bytes_total: total,
                percent,
            },
        );
    }

    drop(file);
    std::fs::rename(&tmp, dest)
        .map_err(|e| format!("Rename {} → {}: {e}", tmp.display(), dest.display()))?;

    eprintln!("[AI-GEN] Done: {}", dest.display());
    Ok(())
}
