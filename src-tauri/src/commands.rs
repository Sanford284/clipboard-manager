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

    // 先同步写入剪切板（确保写入完成）
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(text).map_err(|e| e.to_string())?;
    drop(clipboard);

    // 隐藏窗口
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // 后台线程：等焦点切换后模拟粘贴
    std::thread::spawn(move || {
        // 等待窗口完全隐藏、焦点切回目标应用
        std::thread::sleep(std::time::Duration::from_millis(500));
        simulate_paste();
    });

    Ok(())
}

#[cfg(target_os = "macos")]
fn simulate_paste() {
    use std::process::Command;
    // 使用 osascript 模拟按键，最可靠的方式
    let _ = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to keystroke \"v\" using command down")
        .output();
}

#[cfg(target_os = "windows")]
fn simulate_paste() {
    use enigo::{Enigo, Keyboard, Key, Settings, Direction};
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        enigo.key(Key::Control, Direction::Press).ok();
        enigo.key(Key::Unicode('v'), Direction::Click).ok();
        enigo.key(Key::Control, Direction::Release).ok();
    }
}
