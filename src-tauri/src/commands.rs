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
    app: tauri::AppHandle,
    id: i64,
) -> Result<(), String> {
    use arboard::Clipboard;
    use tauri::Manager;

    // 先从数据库取出内容，尽快释放锁
    let text_to_paste = {
        let db = db.lock().unwrap();
        let items = db.get_items(1000, 0, None, None).map_err(|e| e.to_string())?;
        items.into_iter()
            .find(|i| i.id == id)
            .and_then(|item| item.text_content)
    };

    let text = text_to_paste.ok_or("Item not found or no text content")?;

    // 写入剪切板
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;

    // 隐藏窗口
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // 在新线程中等待焦点切换后模拟粘贴
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));

        use enigo::{Enigo, Keyboard, Key, Settings, Direction};
        if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
            if cfg!(target_os = "macos") {
                enigo.key(Key::Meta, Direction::Press).ok();
                enigo.key(Key::Unicode('v'), Direction::Click).ok();
                enigo.key(Key::Meta, Direction::Release).ok();
            } else {
                enigo.key(Key::Control, Direction::Press).ok();
                enigo.key(Key::Unicode('v'), Direction::Click).ok();
                enigo.key(Key::Control, Direction::Release).ok();
            }
        }
    });

    Ok(())
}
