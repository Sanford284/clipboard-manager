use crate::storage::{models::ClipboardItem, Database};
use tauri::State;
use std::sync::{Arc, Mutex};

#[tauri::command]
pub fn get_clipboard_items(
    db: State<Arc<Mutex<Database>>>,
    limit: u32,
    offset: u32,
    search: Option<String>,
    content_type: Option<String>,
) -> Result<Vec<ClipboardItem>, String> {
    let db = db.lock().unwrap();
    db.get_items(limit, offset, search, content_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_clipboard_item(
    db: State<Arc<Mutex<Database>>>,
    id: i64,
) -> Result<(), String> {
    let db = db.lock().unwrap();
    db.delete_item(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_pin(
    db: State<Arc<Mutex<Database>>>,
    id: i64,
) -> Result<(), String> {
    let db = db.lock().unwrap();
    db.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn paste_item(
    db: State<Arc<Mutex<Database>>>,
    id: i64,
) -> Result<(), String> {
    use arboard::Clipboard;

    let db = db.lock().unwrap();
    let items = db.get_items(1000, 0, None, None).map_err(|e| e.to_string())?;

    if let Some(item) = items.into_iter().find(|i| i.id == id) {
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

        if let Some(text) = item.text_content {
            clipboard.set_text(text).map_err(|e| e.to_string())?;
        }

        Ok(())
    } else {
        Err("Item not found".to_string())
    }
}
