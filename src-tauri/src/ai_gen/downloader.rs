/// Model downloader — fetches weight files from HuggingFace with optional
/// Bearer token authentication and real-time progress events.
///
/// HF Token priority (highest first):
///   1. HUGGING_FACE_HUB_TOKEN or HF_TOKEN environment variable
///   2. Token stored in <models_root>/hf_token (written by the settings UI)
///
/// Download errors are typed so the frontend can distinguish:
///   - AuthRequired  → 401/403: the file needs a HF token
///   - NotFound      → 404: wrong path or repo name
///   - Network       → other errors

use crate::ai_gen::model_registry::ModelDef;
use futures_util::StreamExt;
use serde::Serialize;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

// ── Paths ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub tokenizer: PathBuf,
    pub clip_weights: PathBuf,
    pub unet_weights: PathBuf,
    pub vae_weights: PathBuf,
    pub lora_weights: Option<PathBuf>,
}

pub fn models_root() -> PathBuf {
    if let Ok(dir) = std::env::var("AI_GEN_MODELS_DIR") {
        return PathBuf::from(dir);
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let is_dev = exe_dir
        .components()
        .any(|c| c.as_os_str() == "debug" || c.as_os_str() == "release");

    if is_dev {
        let dev = exe_dir.join("../../ai_models");
        return dev.canonicalize().unwrap_or(dev);
    }
    exe_dir.join("ai_models")
}

pub fn model_dir(model_id: &str) -> PathBuf {
    models_root().join(model_id)
}

fn expected_paths(def: &ModelDef, dir: &Path) -> ModelPaths {
    ModelPaths {
        tokenizer:    dir.join("tokenizer/tokenizer.json"),
        clip_weights: dir.join("text_encoder/model.safetensors"),
        unet_weights: dir.join("unet/diffusion_pytorch_model.safetensors"),
        vae_weights:  dir.join("vae/diffusion_pytorch_model.safetensors"),
        lora_weights: def.lora.as_ref().map(|_| dir.join("lora.safetensors")),
    }
}

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

pub fn get_paths(def: &ModelDef) -> Option<ModelPaths> {
    if !is_downloaded(def) { return None; }
    Some(expected_paths(def, &model_dir(def.id)))
}

// ── HF Token storage ──────────────────────────────────────────────────────────

/// Path of the stored token file.
fn token_path() -> PathBuf {
    models_root().join("hf_token")
}

/// Read the HF access token.
///
/// Order of precedence:
///   1. `HUGGING_FACE_HUB_TOKEN` env var (standard HF variable)
///   2. `HF_TOKEN` env var (shorter alias)
///   3. Token file at `<models_root>/hf_token`
pub fn load_hf_token() -> Option<String> {
    if let Ok(t) = std::env::var("HUGGING_FACE_HUB_TOKEN") {
        if !t.is_empty() { return Some(t.trim().to_string()); }
    }
    if let Ok(t) = std::env::var("HF_TOKEN") {
        if !t.is_empty() { return Some(t.trim().to_string()); }
    }
    std::fs::read_to_string(token_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Persist a HF access token to the token file.
pub fn save_hf_token(token: &str) -> Result<(), String> {
    let root = models_root();
    std::fs::create_dir_all(&root)
        .map_err(|e| format!("Create models dir: {e}"))?;
    std::fs::write(token_path(), token.trim())
        .map_err(|e| format!("Save token: {e}"))
}

/// Delete the stored token file.
pub fn delete_hf_token() -> Result<(), String> {
    let path = token_path();
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| format!("Delete token: {e}"))?;
    }
    Ok(())
}

// ── Progress event ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub model_id: String,
    pub file_name: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub percent: f32,
}

// ── Download error ────────────────────────────────────────────────────────────

/// Structured download error so the frontend can show the right UI.
#[derive(Debug)]
pub enum DownloadError {
    /// HTTP 401 / 403 — needs a HF access token
    AuthRequired { url: String, model_id: String },
    /// HTTP 404 — file path or repo name is wrong
    NotFound { url: String },
    /// Any other error
    Other(String),
}

impl DownloadError {
    /// Convert to the JSON-serialisable shape sent to the frontend.
    pub fn to_payload(&self) -> serde_json::Value {
        match self {
            DownloadError::AuthRequired { url, model_id } => serde_json::json!({
                "kind": "auth_required",
                "url": url,
                "modelId": model_id,
                "hint": "请在设置中填入 HuggingFace Access Token",
            }),
            DownloadError::NotFound { url } => serde_json::json!({
                "kind": "not_found",
                "url": url,
            }),
            DownloadError::Other(msg) => serde_json::json!({
                "kind": "error",
                "message": msg,
            }),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            DownloadError::AuthRequired { url, .. } => {
                format!("需要 HuggingFace Token（401/403）\n地址: {url}\n请在 AI 生成设置中填入 Token")
            }
            DownloadError::NotFound { url } => {
                format!("文件不存在（404）: {url}")
            }
            DownloadError::Other(m) => m.clone(),
        }
    }
}

// ── Download ──────────────────────────────────────────────────────────────────

/// Download all model files for `def`.
///
/// Files already present are skipped. Each chunk emits `"ai-gen:download-progress"`.
/// Auth errors are returned as `Err(DownloadError::AuthRequired)`.
pub async fn download_model(
    app: &AppHandle,
    def: &ModelDef,
) -> Result<ModelPaths, DownloadError> {
    let dir = model_dir(def.id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| DownloadError::Other(format!("Create model dir: {e}")))?;

    let mut files: Vec<(&str, &str, PathBuf)> = vec![
        (def.hf_repo, "tokenizer/tokenizer.json",                 dir.join("tokenizer/tokenizer.json")),
        (def.hf_repo, "text_encoder/model.safetensors",           dir.join("text_encoder/model.safetensors")),
        (def.hf_repo, "unet/diffusion_pytorch_model.safetensors", dir.join("unet/diffusion_pytorch_model.safetensors")),
        (def.hf_repo, "vae/diffusion_pytorch_model.safetensors",  dir.join("vae/diffusion_pytorch_model.safetensors")),
    ];

    if let Some(lora) = &def.lora {
        files.push((lora.hf_repo, lora.filename, dir.join("lora.safetensors")));
    }

    let token = load_hf_token();
    let client = build_client(&token)
        .map_err(|e| DownloadError::Other(format!("Build HTTP client: {e}")))?;

    for (repo, hf_path, local) in &files {
        if local.exists() {
            eprintln!("[AI-GEN] Skip (exists): {}", local.display());
            continue;
        }
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| DownloadError::Other(format!("Create dir: {e}")))?;
        }

        let file_label = local
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(hf_path)
            .to_string();

        let url = hf_url(repo, hf_path);
        eprintln!("[AI-GEN] Downloading {file_label} from {url}");

        download_file(&client, &url, local, app, def.id, &file_label).await?;
    }

    get_paths(def).ok_or_else(|| DownloadError::Other("Files missing after download".into()))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn hf_url(repo: &str, path: &str) -> String {
    let base = std::env::var("HF_ENDPOINT")
        .unwrap_or_else(|_| "https://huggingface.co".into());
    format!("{base}/{repo}/resolve/main/{path}")
}

fn build_client(token: &Option<String>) -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(t) = token {
        if let Ok(val) = reqwest::header::HeaderValue::from_str(&format!("Bearer {t}")) {
            headers.insert(reqwest::header::AUTHORIZATION, val);
        }
    }
    reqwest::Client::builder()
        .user_agent("logo-studio/1.0")
        .timeout(std::time::Duration::from_secs(3600))
        .default_headers(headers)
        .build()
}

async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    app: &AppHandle,
    model_id: &str,
    file_name: &str,
) -> Result<(), DownloadError> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| DownloadError::Other(format!("GET {url}: {e}")))?;

    match resp.status().as_u16() {
        200..=299 => {}
        401 | 403 => {
            return Err(DownloadError::AuthRequired {
                url: url.to_string(),
                model_id: model_id.to_string(),
            });
        }
        404 => {
            return Err(DownloadError::NotFound { url: url.to_string() });
        }
        code => {
            return Err(DownloadError::Other(format!("HTTP {code}: {url}")));
        }
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    let tmp = dest.with_extension("safetensors.tmp");
    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| DownloadError::Other(format!("Create {}: {e}", tmp.display())))?;

    let _ = app.emit("ai-gen:download-progress", DownloadProgress {
        model_id: model_id.into(),
        file_name: file_name.into(),
        bytes_done: 0,
        bytes_total: total,
        percent: 0.0,
    });

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DownloadError::Other(format!("Stream: {e}")))?;
        file.write_all(&chunk)
            .map_err(|e| DownloadError::Other(format!("Write: {e}")))?;
        downloaded += chunk.len() as u64;

        let percent = if total > 0 { downloaded as f32 / total as f32 * 100.0 } else { 0.0 };
        let _ = app.emit("ai-gen:download-progress", DownloadProgress {
            model_id: model_id.into(),
            file_name: file_name.into(),
            bytes_done: downloaded,
            bytes_total: total,
            percent,
        });
    }

    drop(file);
    std::fs::rename(&tmp, dest)
        .map_err(|e| DownloadError::Other(format!("Rename: {e}")))?;

    eprintln!("[AI-GEN] Done: {}", dest.display());
    Ok(())
}
