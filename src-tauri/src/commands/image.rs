use base64::{engine::general_purpose, Engine as _};
use image::ImageFormat;
use serde::Serialize;
use std::io::Cursor;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub data: String, // base64 encoded
}

/// Read an image file and return its metadata + base64 data
#[tauri::command]
pub fn read_image(path: String) -> Result<ImageInfo, String> {
    let img = image::open(&path).map_err(|e| format!("Failed to open image: {e}"))?;

    let format_str = PathBuf::from(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_uppercase();

    let (width, height) = (img.width(), img.height());

    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| format!("Failed to encode image: {e}"))?;

    let b64 = general_purpose::STANDARD.encode(buf.into_inner());

    Ok(ImageInfo {
        width,
        height,
        format: format_str,
        data: b64,
    })
}

/// Save a base64 PNG data URL to a file path
#[tauri::command]
pub fn save_image(data_url: String, path: String) -> Result<(), String> {
    let base64_data = data_url
        .split(',')
        .nth(1)
        .ok_or("Invalid data URL format")?;

    let bytes = general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| format!("Base64 decode error: {e}"))?;

    std::fs::write(&path, &bytes).map_err(|e| format!("Write error: {e}"))?;

    Ok(())
}
