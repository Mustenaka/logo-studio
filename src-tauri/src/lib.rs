mod ai_gen;
mod commands;
mod sam2;

use commands::ai_gen::{
    ai_gen_delete_hf_token, ai_gen_device_info, ai_gen_download, ai_gen_generate,
    ai_gen_get_hf_token, ai_gen_list_models, ai_gen_set_hf_token,
};
use commands::export::export_icon_set;
use commands::image::{read_image, save_image};
use commands::project::{read_text_file, write_text_file};
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
            // existing commands
            read_image,
            save_image,
            export_icon_set,
            segment_image,
            check_sam2,
            // project file I/O
            write_text_file,
            read_text_file,
            // AI image generation
            ai_gen_device_info,
            ai_gen_list_models,
            ai_gen_download,
            ai_gen_generate,
            ai_gen_get_hf_token,
            ai_gen_set_hf_token,
            ai_gen_delete_hf_token,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
