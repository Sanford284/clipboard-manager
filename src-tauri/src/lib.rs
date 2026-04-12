mod clipboard;
mod commands;
mod storage;

use clipboard::{create_monitor, ClipboardContent};
use storage::{models::ClipboardItem, Database};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = std::env::current_dir().unwrap().join("data");
    std::fs::create_dir_all(&app_data_dir).unwrap();
    let db_path = app_data_dir.join("clipboard.db");

    let db = Database::new(db_path).expect("Failed to initialize database");
    let db_state = Arc::new(Mutex::new(db));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(db_state.clone())
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
            app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                    if let Some(window) = app_handle_shortcut.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            window.hide().ok();
                        } else {
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

