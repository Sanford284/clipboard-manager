mod clipboard;
mod commands;
mod storage;

use clipboard::{create_monitor, ClipboardContent};
use storage::{models::ClipboardItem, Database};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 记录唤起剪切板窗口之前的前台应用 bundle id，用于粘贴后恢复焦点
pub type PreviousApp = Arc<Mutex<Option<String>>>;

#[cfg(target_os = "macos")]
fn get_frontmost_app_bundle_id() -> Option<String> {
    use cocoa::foundation::NSString;
    use objc::{msg_send, sel, sel_impl, runtime::Object};
    use std::ffi::CStr;
    unsafe {
        let workspace: *mut Object = msg_send![objc::class!(NSWorkspace), sharedWorkspace];
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() { return None; }
        let bundle_id: *mut Object = msg_send![app, bundleIdentifier];
        if bundle_id.is_null() { return None; }
        let cstr = CStr::from_ptr(bundle_id.UTF8String());
        Some(cstr.to_string_lossy().into_owned())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = std::env::current_dir().unwrap().join("data");
    std::fs::create_dir_all(&app_data_dir).unwrap();
    let db_path = app_data_dir.join("clipboard.db");

    let db = Database::new(db_path).expect("Failed to initialize database");
    let db_state = Arc::new(Mutex::new(db));

    let previous_app: PreviousApp = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(db_state.clone())
        .manage(previous_app.clone())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let db_clone = db_state.clone();

            let mut monitor = create_monitor();

            monitor.start(Box::new(move |content| {
                let db = db_clone.lock().unwrap();

                let item = match content {
                    ClipboardContent::Text(text) => {
                        let preview = if text.len() > 200 {
                            text.chars().take(200).collect::<String>() + "..."
                        } else {
                            text.clone()
                        };

                        ClipboardItem {
                            id: 0,
                            content_type: "text".to_string(),
                            text_content: Some(text.clone()),
                            html_content: None,
                            blob_content: None,
                            file_path: None,
                            preview,
                            app_source: None,
                            pinned: false,
                            created_at: chrono::Utc::now().timestamp_millis(),
                            hash: Database::compute_hash(&text),
                        }
                    }
                    ClipboardContent::Image(data) => {
                        ClipboardItem {
                            id: 0,
                            content_type: "image".to_string(),
                            text_content: None,
                            html_content: None,
                            blob_content: Some(data.clone()),
                            file_path: None,
                            preview: "[Image]".to_string(),
                            app_source: None,
                            pinned: false,
                            created_at: chrono::Utc::now().timestamp_millis(),
                            hash: Database::compute_hash(&format!("{:?}", data)),
                        }
                    }
                    _ => return,
                };

                if let Ok(id) = db.insert_item(&item) {
                    app_handle.emit("clipboard-changed", id).ok();
                }
            })).ok();

            // 注册全局快捷键
            let shortcut_str = if cfg!(target_os = "macos") {
                "CommandOrControl+Shift+V"
            } else {
                "Control+Shift+V"
            };

            let shortcut: tauri_plugin_global_shortcut::Shortcut = shortcut_str.parse().unwrap();

            let app_handle_shortcut = app.handle().clone();
            let previous_app_shortcut = previous_app.clone();
            app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app_handle_shortcut.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            window.hide().ok();
                        } else {
                            // 记录当前前台应用，以便粘贴后恢复焦点
                            #[cfg(target_os = "macos")]
                            {
                                if let Some(bundle_id) = get_frontmost_app_bundle_id() {
                                    *previous_app_shortcut.lock().unwrap() = Some(bundle_id);
                                }
                            }
                            window.show().ok();
                            window.set_focus().ok();
                        }
                    }
                }
            }).ok();

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 关闭窗口时隐藏而不是退出
                window.hide().ok();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_clipboard_items,
            commands::delete_clipboard_item,
            commands::toggle_pin,
            commands::paste_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

