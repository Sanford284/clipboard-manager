pub mod models;

use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_tables()?;
        Ok(db)
    }

    fn init_tables(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                text_content TEXT,
                html_content TEXT,
                blob_content BLOB,
                file_path TEXT,
                preview TEXT NOT NULL,
                app_source TEXT,
                pinned INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                hash TEXT NOT NULL UNIQUE
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_created_at ON clipboard_items(created_at DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_pinned ON clipboard_items(pinned)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_content_type ON clipboard_items(content_type)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    pub fn insert_item(&self, item: &models::ClipboardItem) -> Result<i64> {
        let conn = self.conn.lock().unwrap();

        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM clipboard_items WHERE hash = ?1",
                [&item.hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            conn.execute(
                "UPDATE clipboard_items SET created_at = ?1 WHERE id = ?2",
                [&item.created_at, &id],
            )?;
            return Ok(id);
        }

        conn.execute(
            "INSERT INTO clipboard_items (content_type, text_content, html_content, blob_content, file_path, preview, app_source, pinned, created_at, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                &item.content_type,
                &item.text_content,
                &item.html_content,
                &item.blob_content,
                &item.file_path,
                &item.preview,
                &item.app_source,
                &item.pinned,
                &item.created_at,
                &item.hash,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_items(&self, limit: u32, offset: u32, search: Option<String>, content_type: Option<String>) -> Result<Vec<models::ClipboardItem>> {
        let conn = self.conn.lock().unwrap();

        let mut query = "SELECT id, content_type, text_content, html_content, blob_content, file_path, preview, app_source, pinned, created_at, hash FROM clipboard_items WHERE 1=1".to_string();

        if let Some(ct) = content_type {
            query.push_str(&format!(" AND content_type = '{}'", ct));
        }

        if let Some(s) = search {
            query.push_str(&format!(" AND preview LIKE '%{}%'", s));
        }

        query.push_str(" ORDER BY pinned DESC, created_at DESC LIMIT ?1 OFFSET ?2");

        let mut stmt = conn.prepare(&query)?;
        let items = stmt.query_map([limit, offset], |row| {
            Ok(models::ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                html_content: row.get(3)?,
                blob_content: row.get(4)?,
                file_path: row.get(5)?,
                preview: row.get(6)?,
                app_source: row.get(7)?,
                pinned: row.get::<_, i32>(8)? != 0,
                created_at: row.get(9)?,
                hash: row.get(10)?,
            })
        })?;

        items.collect()
    }

    pub fn delete_item(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_items WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn toggle_pin(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE clipboard_items SET pinned = NOT pinned WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        );
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    pub fn compute_hash(content: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
