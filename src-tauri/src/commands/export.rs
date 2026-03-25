use base64::{engine::general_purpose, Engine as _};
use image::imageops::FilterType;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct IconEntry {
    /// Output pixel size (square)
    pub size: u32,
    /// Relative path from output_dir, may include subdirectories
    /// e.g. "ios/AppIcon-60@3x.png" or "favicon-32x32.png"
    pub relpath: String,
}

/// Generate an icon set from a base64 PNG data URL.
/// Creates all files under `output_dir`, creating subdirectories as needed.
/// Returns the number of files written.
#[tauri::command]
pub fn export_icon_set(
    data_url: String,
    output_dir: String,
    entries: Vec<IconEntry>,
) -> Result<usize, String> {
    // Decode source image
    let b64 = if data_url.starts_with("data:") {
        data_url.split(',').nth(1).ok_or("Invalid data URL")?
    } else {
        &data_url
    };
    let bytes = general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Base64 decode: {e}"))?;
    let src = image::load_from_memory(&bytes).map_err(|e| format!("Image decode: {e}"))?;

    let out_root = Path::new(&output_dir);

    for entry in &entries {
        let target = out_root.join(&entry.relpath);

        // Create parent directories
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Create dir {:?}: {e}", parent))?;
        }

        let resized = src.resize_exact(entry.size, entry.size, FilterType::Lanczos3);
        resized
            .save(&target)
            .map_err(|e| format!("Save {:?}: {e}", target))?;
    }

    Ok(entries.len())
}
