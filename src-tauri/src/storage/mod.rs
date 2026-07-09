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

        Self::ensure_column(&conn, "clipboard_items", "thumb_content", "BLOB")?;

        Ok(())
    }

    fn ensure_column(conn: &Connection, table: &str, col: &str, def: &str) -> Result<()> {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
        let cols: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))?
            .filter_map(|x| x.ok())
            .collect();
        if !cols.iter().any(|c| c == col) {
            conn.execute(&format!("ALTER TABLE {} ADD COLUMN {} {}", table, col, def), [])?;
        }
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
            "INSERT INTO clipboard_items (content_type, text_content, html_content, blob_content, thumb_content, file_path, preview, app_source, pinned, created_at, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                &item.content_type,
                &item.text_content,
                &item.html_content,
                &item.blob_content,
                &item.thumb_content,
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

        let mut query = "SELECT id, content_type, text_content, html_content, thumb_content, file_path, preview, app_source, pinned, created_at, hash FROM clipboard_items WHERE 1=1".to_string();

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
                blob_content: None,
                thumb_content: row.get(4)?,
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

    /// 按 id 取单条（含 blob_content，供 paste 使用）
    pub fn get_item_by_id(&self, id: i64) -> Result<Option<models::ClipboardItem>> {
        let conn = self.conn.lock().unwrap();
        let res = conn.query_row(
            "SELECT id, content_type, text_content, html_content, blob_content, thumb_content, file_path, preview, app_source, pinned, created_at, hash FROM clipboard_items WHERE id = ?1",
            [id],
            |row| Ok(models::ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                html_content: row.get(3)?,
                blob_content: row.get(4)?,
                thumb_content: row.get(5)?,
                file_path: row.get(6)?,
                preview: row.get(7)?,
                app_source: row.get(8)?,
                pinned: row.get::<_, i32>(9)? != 0,
                created_at: row.get(10)?,
                hash: row.get(11)?,
            }),
        );
        match res {
            Ok(item) => Ok(Some(item)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 清空所有非置顶记录
    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_items WHERE pinned = 0", [])?;
        Ok(())
    }

    /// 自动清理：仅保留最近 limit 条非置顶记录
    pub fn enforce_history_limit(&self, limit: u32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM clipboard_items WHERE pinned = 0 AND id NOT IN (
                SELECT id FROM clipboard_items WHERE pinned = 0
                ORDER BY created_at DESC LIMIT ?1
            )",
            [limit],
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

    pub fn compute_hash_bytes(bytes: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db() -> Database {
        let mut p = std::env::temp_dir();
        p.push(format!("cm-test-{}-{}.db", std::process::id(), uuid_like()));
        let _ = std::fs::remove_file(&p);
        Database::new(p).unwrap()
    }
    // 简易唯一后缀（不引入 uuid 依赖）
    fn uuid_like() -> String {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("{}", n)
    }

    fn text_item(preview: &str, ts: i64) -> models::ClipboardItem {
        models::ClipboardItem {
            id: 0, content_type: "text".into(),
            text_content: Some(preview.into()), html_content: None,
            blob_content: None, thumb_content: None, file_path: None,
            preview: preview.into(), app_source: None, pinned: false,
            created_at: ts, hash: Database::compute_hash(preview),
        }
    }

    #[test]
    fn settings_roundtrip() {
        let db = tmp_db();
        assert_eq!(db.get_setting("theme").unwrap(), None);
        db.set_setting("theme", "dark").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("dark".into()));
        db.set_setting("theme", "light").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("light".into()));
    }

    #[test]
    fn migration_is_idempotent() {
        // 同一路径二次打开：第一次建表+迁移，第二次列已存在不应报错
        let mut p = std::env::temp_dir();
        p.push(format!("cm-idem-{}-{}.db", std::process::id(), uuid_like()));
        let _ = std::fs::remove_file(&p);
        {
            let _db1 = Database::new(p.clone()).unwrap();
        }
        let _db2 = Database::new(p.clone()).unwrap();
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn thumb_content_roundtrips() {
        let db = tmp_db();
        let mut it = text_item("img", 1);
        it.thumb_content = Some(vec![1, 2, 3, 4]);
        db.insert_item(&it).unwrap();
        let items = db.get_items(100, 0, None, None).unwrap();
        assert_eq!(items[0].thumb_content, Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn enforce_history_limit_keeps_pinned_and_recent() {
        let db = tmp_db();
        let mut pinned = text_item("pinned", 1);
        pinned.pinned = true;
        db.insert_item(&pinned).unwrap();
        for i in 0..6 {
            let mut it = text_item(&format!("t{}", i), 100 + i);
            it.hash = Database::compute_hash(&format!("t{}", i));
            db.insert_item(&it).unwrap();
        }
        db.enforce_history_limit(3).unwrap();
        let items = db.get_items(100, 0, None, None).unwrap();
        // pinned + 最近 3 条非置顶 = 4
        assert_eq!(items.len(), 4);
        assert!(items.iter().any(|i| i.preview == "pinned" && i.pinned));
    }

    #[test]
    fn clear_history_keeps_pinned() {
        let db = tmp_db();
        let mut pinned = text_item("pinned", 1);
        pinned.pinned = true;
        db.insert_item(&pinned).unwrap();
        db.insert_item(&text_item("a", 2)).unwrap();
        db.clear_history().unwrap();
        let items = db.get_items(100, 0, None, None).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].preview, "pinned");
    }

    #[test]
    fn get_item_by_id_returns_blob() {
        let db = tmp_db();
        let mut it = text_item("hello", 5);
        it.blob_content = Some(vec![1, 2, 3]);
        let id = db.insert_item(&it).unwrap();
        let got = db.get_item_by_id(id).unwrap().unwrap();
        assert_eq!(got.blob_content, Some(vec![1, 2, 3]));
        assert_eq!(got.text_content.as_deref(), Some("hello"));
    }

    #[test]
    fn get_items_omits_blob() {
        let db = tmp_db();
        let mut it = text_item("hello", 5);
        it.blob_content = Some(vec![9, 9, 9]);
        db.insert_item(&it).unwrap();
        let items = db.get_items(100, 0, None, None).unwrap();
        assert_eq!(items[0].blob_content, None); // 列表不返回原图
    }
}
