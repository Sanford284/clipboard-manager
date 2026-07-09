use crate::storage::{models::ClipboardItem, Database};
use crate::PreviousApp;
use tauri::State;
use std::collections::HashMap;
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

    // 按 id 取条目（含 blob_content），尽快释放锁
    let item = {
        let db = db.lock().unwrap();
        db.get_item_by_id(id).map_err(|e| e.to_string())?
            .ok_or("Item not found")?
    };

    // 写入剪切板：图片走 set_image，其余走 set_text
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    if item.content_type == "image" {
        if let Some(blob) = item.blob_content.as_ref() {
            let img = image::load_from_memory(blob).map_err(|e| e.to_string())?.to_rgba8();
            let data = arboard::ImageData {
                width: img.width() as usize,
                height: img.height() as usize,
                bytes: img.into_raw().into(),
            };
            clipboard.set_image(data).map_err(|e| e.to_string())?;
        } else {
            return Err("Image item has no blob".into());
        }
    } else {
        let text = item.text_content.unwrap_or_default();
        clipboard.set_text(text).map_err(|e| e.to_string())?;
    }
    drop(clipboard);

    // 取出之前记录的前台应用名称
    let target_app = previous_app.lock().unwrap().take();
    eprintln!("[paste_item] id={id} target_app={:?}", target_app);

    // 隐藏窗口
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        eprintln!("[paste_item] main window hide() called");
    } else {
        eprintln!("[paste_item] WARNING: main window not found");
    }

    // 后台线程：激活目标应用后模拟粘贴
    std::thread::spawn(move || {
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

fn simulate_paste() {
    use enigo::{Enigo, Keyboard, Key, Settings, Direction};
    // 直接在本进程用 CGEvent(macOS)/Win32(Windows) 注入按键，权限归属明确
    // （macOS 上归属本 app，配合启动时的辅助功能授权弹窗，授权后即时生效）。
    if let Ok(mut enigo) = Enigo::new(&Settings::default()) {
        // macOS 用 Meta(Command)，Windows 用 Control
        let modifier = if cfg!(target_os = "macos") {
            Key::Meta
        } else {
            Key::Control
        };
        enigo.key(modifier, Direction::Press).ok();
        enigo.key(Key::Unicode('v'), Direction::Click).ok();
        enigo.key(modifier, Direction::Release).ok();
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

#[tauri::command]
pub fn get_settings(
    db: State<Arc<Mutex<Database>>>,
) -> Result<HashMap<String, String>, String> {
    let db = db.lock().unwrap();
    let keys = [
        "theme", "window_width", "window_height", "show_source",
        "history_mode", "history_limit", "autostart", "shortcut",
    ];
    let mut map = HashMap::new();
    for k in keys {
        if let Ok(Some(v)) = db.get_setting(k) {
            map.insert((*k).to_string(), v);
        }
    }
    Ok(map)
}

#[tauri::command]
pub fn set_setting(
    db: State<Arc<Mutex<Database>>>,
    app: tauri::AppHandle,
    key: String,
    value: String,
) -> Result<(), String> {
    use tauri::Emitter;
    {
        let db = db.lock().unwrap();
        db.set_setting(&key, &value).map_err(|e| e.to_string())?;
    }
    // 通知所有 webview 重新加载设置（主窗口据此即时切换主题等）
    let _ = app.emit("settings-changed", ());
    Ok(())
}

#[tauri::command]
pub fn clear_history(
    db: State<Arc<Mutex<Database>>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri::Emitter;
    {
        let db = db.lock().unwrap();
        db.clear_history().map_err(|e| e.to_string())?;
    }
    // 通知主窗口刷新列表
    let _ = app.emit("clipboard-changed", ());
    Ok(())
}

#[tauri::command]
pub fn set_autostart(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if enabled {
        al.enable().map_err(|e| e.to_string())?;
    } else {
        al.disable().map_err(|e| e.to_string())?;
    }
    Ok(enabled)
}
