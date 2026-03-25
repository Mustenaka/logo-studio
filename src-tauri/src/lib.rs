mod commands;
mod sam2;

use commands::export::export_icon_set;
use commands::image::{read_image, save_image};
use commands::segment::segment_image;

#[tauri::command]
fn check_sam2() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let available = sam2::is_available();
    format!("exe={exe} | sam2_available={available}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            read_image,
            save_image,
            export_icon_set,
            segment_image,
            check_sam2,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
