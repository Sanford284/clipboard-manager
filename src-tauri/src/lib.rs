mod clipboard;
mod commands;
mod storage;

use clipboard::{create_monitor, ClipboardContent};
use storage::{models::ClipboardItem, Database};
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_data_dir = std::env::current_dir().unwrap().join("data");
    std::fs::create_dir_all(&app_data_dir).unwrap();
    let db_path = app_data_dir.join("clipboard.db");

    let db = Database::new(db_path).expect("Failed to initialize database");
    let db_state = Mutex::new(db);

    tauri::Builder::default()
        .manage(db_state)
        .setup(|app| {
            let app_handle = app.handle().clone();
            let db = app.state::<Mutex<Database>>();

            let mut monitor = create_monitor();

            monitor.start(Box::new(move |content| {
                let db = db.lock().unwrap();

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

            Ok(())
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

