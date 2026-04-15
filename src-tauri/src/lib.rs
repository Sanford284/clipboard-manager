mod clipboard;
mod commands;
mod storage;

use clipboard::{create_monitor, ClipboardContent};
use commands::MonitorPaused;
use storage::{models::ClipboardItem, Database};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

/// 记录唤起剪切板窗口之前的前台应用 bundle id，用于粘贴后恢复焦点
pub type PreviousApp = Arc<Mutex<Option<String>>>;

#[cfg(target_os = "macos")]
pub fn get_frontmost_app_bundle_id() -> Option<String> {
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

fn register_shortcut(app: &tauri::AppHandle, shortcut_str: &str, previous_app: PreviousApp) -> Result<(), String> {
    let shortcut: tauri_plugin_global_shortcut::Shortcut = shortcut_str.parse()
        .map_err(|e: <tauri_plugin_global_shortcut::Shortcut as std::str::FromStr>::Err| e.to_string())?;

    let app_handle = app.clone();
    app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            if let Some(window) = app_handle.get_webview_window("main") {
                if window.is_visible().unwrap_or(false) {
                    window.hide().ok();
                } else {
                    #[cfg(target_os = "macos")]
                    {
                        if let Some(bundle_id) = get_frontmost_app_bundle_id() {
                            *previous_app.lock().unwrap() = Some(bundle_id);
                        }
                    }
                    window.show().ok();
                    window.set_focus().ok();
                }
            }
        }
    }).map_err(|e| e.to_string())?;

    Ok(())
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

            // --- Clipboard Monitor ---
            let mut monitor = create_monitor();
            let paused_flag = monitor.paused_flag();

            // Store the paused flag as managed state
            app.manage::<MonitorPaused>(paused_flag.clone());

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

            // --- Read shortcut from DB ---
            let db_for_shortcut = app.state::<Arc<Mutex<Database>>>();
            let shortcut_str = {
                let db = db_for_shortcut.lock().unwrap();
                db.get_setting("shortcut")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| {
                        if cfg!(target_os = "macos") {
                            "CommandOrControl+Shift+V".to_string()
                        } else {
                            "Control+Shift+V".to_string()
                        }
                    })
            };

            // --- Register global shortcut ---
            register_shortcut(app.handle(), &shortcut_str, previous_app.clone())?;

            // --- System Tray ---
            let monitoring_item = CheckMenuItemBuilder::with_id("monitoring", "剪贴板监听")
                .checked(true)
                .build(app)?;

            let open_item = MenuItemBuilder::with_id("open", format!("打开剪贴板  ({})", shortcut_str))
                .build(app)?;

            let shortcut_item = MenuItemBuilder::with_id("shortcut_settings", "修改快捷键...")
                .build(app)?;

            let separator = PredefinedMenuItem::separator(app)?;
            let separator2 = PredefinedMenuItem::separator(app)?;

            let quit_item = MenuItemBuilder::with_id("quit", "退出")
                .build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&monitoring_item)
                .item(&separator)
                .item(&open_item)
                .item(&shortcut_item)
                .item(&separator2)
                .item(&quit_item)
                .build()?;

            let tray_icon = app.default_window_icon().cloned()
                .expect("No default window icon found");

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .menu(&menu)
                .menu_on_left_click(true)
                .tooltip("Clipboard Manager")
                .on_menu_event(move |app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
                    match event.id().as_ref() {
                        "monitoring" => {
                            let paused = app.state::<MonitorPaused>();
                            let was_paused = paused.load(Ordering::SeqCst);
                            paused.store(!was_paused, Ordering::SeqCst);
                            // Toggle the check mark
                            monitoring_item.set_checked(was_paused).ok();
                        }
                        "open" => {
                            if let Some(window) = app.get_webview_window("main") {
                                #[cfg(target_os = "macos")]
                                {
                                    if let Some(bundle_id) = get_frontmost_app_bundle_id() {
                                        if let Some(prev) = app.try_state::<PreviousApp>() {
                                            *prev.lock().unwrap() = Some(bundle_id);
                                        }
                                    }
                                }
                                window.show().ok();
                                window.set_focus().ok();
                            }
                        }
                        "shortcut_settings" => {
                            // Create or focus the settings window
                            if let Some(window) = app.get_webview_window("settings") {
                                window.set_focus().ok();
                            } else {
                                let _settings_window = tauri::WebviewWindowBuilder::new(
                                    app,
                                    "settings",
                                    tauri::WebviewUrl::App("index.html?window=settings".into()),
                                )
                                .title("修改快捷键")
                                .inner_size(400.0, 250.0)
                                .resizable(false)
                                .center()
                                .build()
                                .ok();
                            }
                        }
                        "quit" => {
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // 关闭窗口时隐藏而不是退出
                    window.hide().ok();
                    api.prevent_close();
                }
                tauri::WindowEvent::Focused(false) => {
                    // 仅对主窗口：失焦时隐藏
                    if window.label() == "main" {
                        window.hide().ok();
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_clipboard_items,
            commands::delete_clipboard_item,
            commands::toggle_pin,
            commands::paste_item,
            commands::get_shortcut,
            commands::set_shortcut,
            commands::toggle_monitoring,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
