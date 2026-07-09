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

/// 当前前台应用的本地化名称（用于显示来源）；取不到返回 None
#[cfg(target_os = "macos")]
pub fn get_frontmost_app_name() -> Option<String> {
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl, runtime::Object};
    use std::ffi::CStr;
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let app: *mut Object = msg_send![workspace, frontmostApplication];
        if app.is_null() { return None; }
        let name: *mut Object = msg_send![app, localizedName];
        if name.is_null() { return None; }
        let cstr = CStr::from_ptr(name.UTF8String());
        Some(cstr.to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "windows")]
pub fn get_frontmost_app_name() -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
    unsafe {
        let hwnd = GetForegroundWindow().ok()?;
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let pwstr = windows::core::PWSTR(buf.as_mut_ptr());
        QueryFullProcessImageNameW(handle, windows::Win32::System::Threading::PROCESS_NAME_FLAGS(0), pwstr, &mut len).ok()?;
        let _ = CloseHandle(handle);
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }
}

/// 单张图片上限（编码后 PNG 字节）。超过则跳过，防止 DB 膨胀。
const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// 由 RGBA 原图生成最大边 256px 的 JPEG 缩略图字节
fn make_thumbnail(rgba: &image::RgbaImage) -> Vec<u8> {
    let (w, h) = rgba.dimensions();
    let max_edge = 256u32;
    let (nw, nh) = if w >= h {
        (max_edge, ((h as f32) * (max_edge as f32) / (w as f32)) as u32)
    } else {
        (((w as f32) * (max_edge as f32) / (h as f32)) as u32, max_edge)
    };
    let thumb = image::imageops::resize(rgba, nw.max(1), nh.max(1), image::imageops::FilterType::Lanczos3);
    let rgb = image::DynamicImage::ImageRgba8(thumb).to_rgb8();
    let mut buf = std::io::Cursor::new(Vec::new());
    let _ = image::DynamicImage::ImageRgb8(rgb).write_to(&mut buf, image::ImageFormat::Jpeg);
    buf.into_inner()
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
    let previous_app: PreviousApp = Arc::new(Mutex::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(previous_app.clone())
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // --- Database ---
            // Use the OS app-data dir, NOT the CWD. A bundled .app launched by
            // LaunchServices runs with CWD "/" (or the home dir), so a CWD-relative
            // path would either crash on create_dir_all or land in an odd place.
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("clipboard.db");
            let db = Database::new(db_path)?;
            let db_state = Arc::new(Mutex::new(db));
            app.manage(db_state.clone());

            let db_clone = db_state.clone();

            // --- Clipboard Monitor ---
            let mut monitor = create_monitor();
            let paused_flag = monitor.paused_flag();

            // Store the paused flag as managed state
            app.manage::<MonitorPaused>(paused_flag.clone());

            monitor.start(Box::new(move |content| {
                let db = db_clone.lock().unwrap();

                let app_source = get_frontmost_app_name();

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
                            thumb_content: None,
                            file_path: None,
                            preview,
                            app_source,
                            pinned: false,
                            created_at: chrono::Utc::now().timestamp_millis(),
                            hash: Database::compute_hash(&text),
                        }
                    }
                    ClipboardContent::Image(img) => {
                        // 由原始 RGBA 构图
                        let rgba = match image::RgbaImage::from_raw(
                            img.width as u32, img.height as u32, img.bytes,
                        ) {
                            Some(r) => r,
                            None => return,
                        };
                        // 编码 PNG 作为原图
                        let mut png_buf = std::io::Cursor::new(Vec::new());
                        if image::DynamicImage::ImageRgba8(rgba.clone())
                            .write_to(&mut png_buf, image::ImageFormat::Png)
                            .is_err()
                        {
                            return;
                        }
                        let png_bytes = png_buf.into_inner();
                        if png_bytes.len() > MAX_IMAGE_BYTES {
                            return; // 超限跳过
                        }
                        let thumb = make_thumbnail(&rgba);
                        let preview = format!("图片 {}×{}", img.width, img.height);

                        ClipboardItem {
                            id: 0,
                            content_type: "image".to_string(),
                            text_content: None,
                            html_content: None,
                            blob_content: Some(png_bytes.clone()),
                            thumb_content: Some(thumb),
                            file_path: None,
                            preview,
                            app_source,
                            pinned: false,
                            created_at: chrono::Utc::now().timestamp_millis(),
                            hash: Database::compute_hash_bytes(&png_bytes),
                        }
                    }
                    _ => return,
                };

                if let Ok(id) = db.insert_item(&item) {
                    app_handle.emit("clipboard-changed", id).ok();
                }

                // 自动清理历史（仅 history_mode=auto）
                let mode = db.get_setting("history_mode").ok().flatten().unwrap_or_else(|| "never".into());
                if mode == "auto" {
                    let limit: u32 = db
                        .get_setting("history_limit").ok().flatten()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(500);
                    let _ = db.enforce_history_limit(limit);
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

            // --- Apply default window size from settings (window is created once at startup) ---
            if let Some(window) = app.get_webview_window("main") {
                let db = db_for_shortcut.lock().unwrap();
                let w = db
                    .get_setting("window_width").ok().flatten()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(800.0)
                    .clamp(320.0, 1200.0);
                let h = db
                    .get_setting("window_height").ok().flatten()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(600.0)
                    .clamp(400.0, 1400.0);
                drop(db);
                let _ = window.set_size(tauri::LogicalSize::new(w, h));
            }

            // --- System Tray ---
            let monitoring_item = CheckMenuItemBuilder::with_id("monitoring", "剪贴板监听")
                .checked(true)
                .build(app)?;

            let open_item = MenuItemBuilder::with_id("open", format!("打开剪贴板  ({})", shortcut_str))
                .build(app)?;

            let shortcut_item = MenuItemBuilder::with_id("shortcut_settings", "设置...")
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
                .show_menu_on_left_click(true)
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
                                .title("设置")
                                .inner_size(560.0, 640.0)
                                .resizable(true)
                                .center()
                                .focused(true)
                                .always_on_top(true)
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
                    // 仅对主窗口：关闭时隐藏而不是退出
                    if window.label() == "main" {
                        window.hide().ok();
                        api.prevent_close();
                    }
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
            commands::get_settings,
            commands::set_setting,
            commands::clear_history,
            commands::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
