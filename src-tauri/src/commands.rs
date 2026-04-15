use crate::storage::{models::ClipboardItem, Database};
use crate::PreviousApp;
use tauri::State;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared type for the clipboard monitor paused flag
pub type MonitorPaused = Arc<AtomicBool>;

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
    previous_app: State<PreviousApp>,
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

    // 取出之前记录的前台应用名称
    let target_app = previous_app.lock().unwrap().take();

    // 隐藏窗口
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }

    // 后台线程：激活目标应用后模拟粘贴
    std::thread::spawn(move || {
        // 激活之前的前台应用（osascript 是同步阻塞的，返回即表示激活完成）
        #[cfg(target_os = "macos")]
        if let Some(app_name) = &target_app {
            activate_app(app_name);
        }

        simulate_paste();
    });

    Ok(())
}

#[cfg(target_os = "macos")]
fn activate_app(bundle_id: &str) {
    use cocoa::base::nil;
    use cocoa::foundation::NSString;
    use objc::{msg_send, sel, sel_impl, runtime::Object, class};
    unsafe {
        let ns_bundle_id = NSString::alloc(nil).init_str(bundle_id);
        let running_apps: *mut Object = msg_send![
            class!(NSRunningApplication),
            runningApplicationsWithBundleIdentifier: ns_bundle_id
        ];
        let count: usize = msg_send![running_apps, count];
        if count > 0 {
            let app: *mut Object = msg_send![running_apps, firstObject];
            // NSApplicationActivateIgnoringOtherApps = 1 << 1 = 2
            let _: bool = msg_send![app, activateWithOptions: 2usize];
        }
    }
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

#[tauri::command]
pub fn get_shortcut(
    db: State<Arc<Mutex<Database>>>,
) -> Result<String, String> {
    let db = db.lock().unwrap();
    let shortcut = db.get_setting("shortcut")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| {
            if cfg!(target_os = "macos") {
                "CommandOrControl+Shift+V".to_string()
            } else {
                "Control+Shift+V".to_string()
            }
        });
    Ok(shortcut)
}

#[tauri::command]
pub fn set_shortcut(
    db: State<Arc<Mutex<Database>>>,
    app: tauri::AppHandle,
    shortcut: String,
) -> Result<(), String> {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    // Validate the new shortcut can be parsed
    let new_shortcut = shortcut.parse::<tauri_plugin_global_shortcut::Shortcut>()
        .map_err(|e| e.to_string())?;

    // Get the old shortcut to unregister it
    let old_shortcut_str = {
        let db = db.lock().unwrap();
        db.get_setting("shortcut")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    "CommandOrControl+Shift+V".to_string()
                } else {
                    "Control+Shift+V".to_string()
                }
            })
    };

    // Unregister old shortcut
    if let Ok(old_shortcut) = old_shortcut_str.parse::<tauri_plugin_global_shortcut::Shortcut>() {
        app.global_shortcut().unregister(old_shortcut).ok();
    }

    // Register new shortcut with the same handler
    let app_handle = app.clone();
    app.global_shortcut().on_shortcut(new_shortcut, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            if let Some(window) = app_handle.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    window.hide().ok();
                } else {
                    #[cfg(target_os = "macos")]
                    {
                        if let Some(bundle_id) = crate::get_frontmost_app_bundle_id() {
                            if let Some(prev) = app_handle.try_state::<crate::PreviousApp>() {
                                *prev.lock().unwrap() = Some(bundle_id);
                            }
                        }
                    }
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
        }
    }).map_err(|e| e.to_string())?;

    // Save to database
    {
        let db = db.lock().unwrap();
        db.set_setting("shortcut", &shortcut).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn toggle_monitoring(
    paused: State<MonitorPaused>,
) -> Result<bool, String> {
    let was_paused = paused.load(Ordering::SeqCst);
    paused.store(!was_paused, Ordering::SeqCst);
    // Return true if monitoring is now active (not paused)
    Ok(was_paused)
}
