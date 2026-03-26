/// Write a UTF-8 text file (used for .lsp project files and .svg exports)
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content.as_bytes())
        .map_err(|e| format!("Write error: {e}"))
}

/// Read a UTF-8 text file (used for .lsp project files)
#[tauri::command]
pub fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path)
        .map_err(|e| format!("Read error: {e}"))
}
