# 剪切板管理工具实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个跨平台剪切板历史管理工具，支持 macOS 和 Windows，提供剪切板监听、历史记录、搜索、预览和跨设备同步功能。

**Architecture:** 使用 Tauri 2.x 构建桌面应用，Rust 后端负责剪切板监听和 SQLite 存储，React + MobX 前端提供响应式 UI。采用事件驱动架构，剪切板变化通过 Tauri event 实时通知前端更新。

**Tech Stack:** Tauri 2.x, Rust, React 18, TypeScript, TailwindCSS, MobX 6, SQLite (rusqlite), arboard, tauri-plugin-global-shortcut

---

## 阶段 1：项目初始化

### 任务 1.1：创建 Tauri 项目基础结构

- [ ] 初始化 Tauri + React + TypeScript 项目

```bash
cd /Users/real/cvte/clipboard-manager
npm create tauri-app@latest . -- --template react-ts --manager npm
```

预期输出：生成 `package.json`, `src/`, `src-tauri/` 等目录结构

提交：
```bash
git init
git add .
git commit -m "chore: initialize Tauri React TypeScript project"
```

---

### 任务 1.2：安装前端依赖

- [ ] 安装 TailwindCSS, MobX 和其他前端依赖

```bash
cd /Users/real/cvte/clipboard-manager
npm install tailwindcss postcss autoprefixer mobx mobx-react-lite @tauri-apps/api
npm install -D @types/node
npx tailwindcss init -p
```

- [ ] 配置 `tailwind.config.js`

```javascript
/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
```

- [ ] 更新 `src/styles.css` 为 TailwindCSS

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

测试命令：
```bash
npm run dev
```

预期输出：开发服务器启动，浏览器打开显示默认 Tauri 页面

提交：
```bash
git add .
git commit -m "chore: add TailwindCSS and MobX dependencies"
```

---

### 任务 1.3：配置 Tauri 窗口和权限

- [ ] 更新 `src-tauri/tauri.conf.json` 配置主窗口

```json
{
  "productName": "Clipboard Manager",
  "version": "0.1.0",
  "identifier": "com.clipboard.manager",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Clipboard Manager",
        "width": 800,
        "height": 600,
        "resizable": true,
        "fullscreen": false,
        "decorations": true,
        "alwaysOnTop": false,
        "visible": false,
        "center": true
      }
    ],
    "security": {
      "csp": null
    }
  }
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager
npm run tauri dev
```

预期输出：Tauri 窗口启动，尺寸为 800x600

提交：
```bash
git add src-tauri/tauri.conf.json
git commit -m "config: configure Tauri window settings"
```

---

### 任务 1.4：添加 Rust 依赖

- [ ] 更新 `src-tauri/Cargo.toml` 添加必要的 crates

```toml
[package]
name = "clipboard-manager"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2.0", features = [] }

[dependencies]
tauri = { version = "2.0", features = ["devtools"] }
tauri-plugin-global-shortcut = "2.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
arboard = "3.3"
tokio = { version = "1", features = ["full"] }
sha2 = "0.10"
chrono = "0.4"

[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_System_DataExchange",
    "Win32_UI_WindowsAndMessaging",
] }

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"
objc = "0.2"
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：所有依赖成功下载和编译，无错误

提交：
```bash
git add src-tauri/Cargo.toml
git commit -m "chore: add Rust dependencies for clipboard monitoring and storage"
```

---

## 阶段 2：SQLite 数据库层

### 任务 2.1：创建数据模型

- [ ] 创建 `src-tauri/src/storage/models.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub text_content: Option<String>,
    pub html_content: Option<String>,
    pub blob_content: Option<Vec<u8>>,
    pub file_path: Option<String>,
    pub preview: String,
    pub app_source: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    RichText,
    Image,
    FilePath,
}

impl ContentType {
    pub fn as_str(&self) -> &str {
        match self {
            ContentType::Text => "text",
            ContentType::RichText => "rich_text",
            ContentType::Image => "image",
            ContentType::FilePath => "file_path",
        }
    }
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：编译成功，无错误

提交：
```bash
git add src-tauri/src/storage/models.rs
git commit -m "feat: add clipboard item data models"
```

---

### 任务 2.2：实现数据库初始化

- [ ] 创建 `src-tauri/src/storage/mod.rs`

```rust
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
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：编译成功

提交：
```bash
git add src-tauri/src/storage/mod.rs
git commit -m "feat: implement database initialization with SQLite"
```

---

### 任务 2.3：实现数据库 CRUD 操作

- [ ] 在 `src-tauri/src/storage/mod.rs` 添加 CRUD 方法

```rust
use models::ClipboardItem;
use sha2::{Sha256, Digest};

impl Database {
    pub fn insert_item(&self, item: &ClipboardItem) -> Result<i64> {
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

    pub fn get_items(&self, limit: u32, offset: u32, search: Option<String>, content_type: Option<String>) -> Result<Vec<ClipboardItem>> {
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
            Ok(ClipboardItem {
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

    pub fn compute_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：编译成功

提交：
```bash
git add src-tauri/src/storage/mod.rs
git commit -m "feat: implement database CRUD operations"
```

---

## 阶段 3：剪切板监听

### 任务 3.1：定义跨平台接口

- [ ] 创建 `src-tauri/src/clipboard/mod.rs`

```rust
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    Text(String),
    RichText { plain: String, html: String },
    Image(Vec<u8>),
    FilePath(String),
}

pub trait ClipboardMonitor: Send {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
}

#[cfg(target_os = "macos")]
pub fn create_monitor() -> Box<dyn ClipboardMonitor> {
    Box::new(macos::MacOSClipboardMonitor::new())
}

#[cfg(target_os = "windows")]
pub fn create_monitor() -> Box<dyn ClipboardMonitor> {
    Box::new(windows::WindowsClipboardMonitor::new())
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：编译成功

提交：
```bash
git add src-tauri/src/clipboard/mod.rs
git commit -m "feat: define cross-platform clipboard monitor interface"
```

---

### 任务 3.2：实现 macOS 剪切板监听

- [ ] 创建 `src-tauri/src/clipboard/macos.rs`

```rust
use super::{ClipboardContent, ClipboardMonitor};
use arboard::Clipboard;
use cocoa::appkit::NSPasteboard;
use cocoa::base::{id, nil};
use cocoa::foundation::NSAutoreleasePool;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct MacOSClipboardMonitor {
    last_change_count: Arc<AtomicI64>,
    running: Arc<AtomicBool>,
}

impl MacOSClipboardMonitor {
    pub fn new() -> Self {
        Self {
            last_change_count: Arc::new(AtomicI64::new(-1)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn get_change_count() -> i64 {
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let pasteboard: id = NSPasteboard::generalPasteboard(nil);
            cocoa::appkit::NSPasteboard::changeCount(pasteboard)
        }
    }
}

impl ClipboardMonitor for MacOSClipboardMonitor {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let last_change_count = Arc::clone(&self.last_change_count);

        thread::spawn(move || {
            let mut clipboard = Clipboard::new().unwrap();
            while running.load(Ordering::SeqCst) {
                let current_count = Self::get_change_count();
                let last_count = last_change_count.load(Ordering::SeqCst);

                if current_count != last_count {
                    last_change_count.store(current_count, Ordering::SeqCst);

                    if let Ok(text) = clipboard.get_text() {
                        callback(ClipboardContent::Text(text));
                    } else if let Ok(image) = clipboard.get_image() {
                        let rgba = image.bytes.to_vec();
                        callback(ClipboardContent::Image(rgba));
                    }
                }

                thread::sleep(Duration::from_secs(1));
            }
        });

        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：编译成功（仅在 macOS 上）

提交：
```bash
git add src-tauri/src/clipboard/macos.rs
git commit -m "feat: implement macOS clipboard monitoring"
```

---

### 任务 3.3：实现 Windows 剪切板监听

- [ ] 创建 `src-tauri/src/clipboard/windows.rs`

```rust
use super::{ClipboardContent, ClipboardMonitor};
use arboard::Clipboard;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct WindowsClipboardMonitor {
    running: Arc<AtomicBool>,
}

impl WindowsClipboardMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ClipboardMonitor for WindowsClipboardMonitor {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send + Sync>) -> Result<(), String> {
        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);

        thread::spawn(move || {
            let mut clipboard = Clipboard::new().unwrap();
            let mut last_text = String::new();

            while running.load(Ordering::SeqCst) {
                if let Ok(text) = clipboard.get_text() {
                    if text != last_text {
                        last_text = text.clone();
                        callback(ClipboardContent::Text(text));
                    }
                }
                thread::sleep(Duration::from_secs(1));
            }
        });

        Ok(())
    }

    fn stop(&mut self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：编译成功

提交：
```bash
git add src-tauri/src/clipboard/windows.rs
git commit -m "feat: implement Windows clipboard monitoring"
```

---

## 阶段 4：Tauri Commands 和事件系统

### 任务 4.1：创建 Tauri commands

- [ ] 创建 `src-tauri/src/commands.rs`

```rust
use crate::storage::{models::ClipboardItem, Database};
use tauri::State;
use std::sync::Mutex;

#[tauri::command]
pub fn get_clipboard_items(
    db: State<Mutex<Database>>,
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
    db: State<Mutex<Database>>,
    id: i64,
) -> Result<(), String> {
    let db = db.lock().unwrap();
    db.delete_item(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_pin(
    db: State<Mutex<Database>>,
    id: i64,
) -> Result<(), String> {
    let db = db.lock().unwrap();
    db.toggle_pin(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn paste_item(
    db: State<Mutex<Database>>,
    id: i64,
) -> Result<(), String> {
    use arboard::Clipboard;

    let db = db.lock().unwrap();
    let items = db.get_items(1000, 0, None, None).map_err(|e| e.to_string())?;

    if let Some(item) = items.into_iter().find(|i| i.id == id) {
        let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;

        if let Some(text) = item.text_content {
            clipboard.set_text(text).map_err(|e| e.to_string())?;
        }

        Ok(())
    } else {
        Err("Item not found".to_string())
    }
}
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager/src-tauri
cargo check
```

预期输出：编译成功

提交：
```bash
git add src-tauri/src/commands.rs
git commit -m "feat: implement Tauri commands for clipboard operations"
```

---

### 任务 4.2：集成到 main.rs

- [ ] 更新 `src-tauri/src/main.rs`

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod commands;
mod storage;

use clipboard::{create_monitor, ClipboardContent};
use storage::{models::ClipboardItem, Database};
use std::sync::Mutex;
use tauri::Manager;

fn main() {
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
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager
npm run tauri dev
```

预期输出：应用启动，剪切板监听开始工作

提交：
```bash
git add src-tauri/src/main.rs
git commit -m "feat: integrate clipboard monitor and commands in main"
```

---

## 阶段 5：全局快捷键

### 任务 5.1：添加全局快捷键

- [ ] 更新 `src-tauri/src/main.rs` 添加快捷键支持

在 `tauri::Builder::default()` 后添加：

```rust
.plugin(tauri_plugin_global_shortcut::Builder::new().build())
```

在 `.setup(|app| {` 内部末尾添加：

```rust
    // 注册全局快捷键
    let shortcut = if cfg!(target_os = "macos") {
        "CommandOrControl+Shift+V"
    } else {
        "Control+Shift+V"
    };

    let app_handle_shortcut = app.handle().clone();
    app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
        if let Some(window) = app_handle_shortcut.get_webview_window("main") {
            if window.is_visible().unwrap_or(false) {
                window.hide().ok();
            } else {
                window.show().ok();
                window.set_focus().ok();
            }
        }
    }).ok();

    app.global_shortcut().register(tauri_plugin_global_shortcut::Shortcut::new(shortcut, None)).ok();
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager
npm run tauri dev
```

预期输出：按 Cmd+Shift+V 可以显示/隐藏窗口

提交：
```bash
git add src-tauri/src/main.rs
git commit -m "feat: add global shortcut to toggle window"
```

---

## 阶段 6：前端 UI

### 任务 6.1：创建 MobX Store

- [ ] 创建 `src/stores/ClipboardStore.ts`

```typescript
import { makeAutoObservable } from 'mobx';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface ClipboardItem {
  id: number;
  contentType: string;
  textContent?: string;
  htmlContent?: string;
  blobContent?: number[];
  filePath?: string;
  preview: string;
  appSource?: string;
  pinned: boolean;
  createdAt: number;
  hash: string;
}

export type ContentType = 'text' | 'rich_text' | 'image' | 'file_path' | 'all';

class ClipboardStore {
  items: ClipboardItem[] = [];
  searchQuery: string = '';
  filterType: ContentType = 'all';
  selectedId: number | null = null;

  constructor() {
    makeAutoObservable(this);
    this.init();
  }

  async init() {
    await this.loadItems();
    listen('clipboard-changed', () => {
      this.loadItems();
    });
  }

  get filteredItems(): ClipboardItem[] {
    return this.items
      .filter(item => {
        if (this.filterType !== 'all' && item.contentType !== this.filterType) return false;
        if (this.searchQuery && !item.preview.toLowerCase().includes(this.searchQuery.toLowerCase())) return false;
        return true;
      })
      .sort((a, b) => {
        if (a.pinned !== b.pinned) return b.pinned ? 1 : -1;
        return b.createdAt - a.createdAt;
      });
  }

  async loadItems() {
    try {
      const items = await invoke<ClipboardItem[]>('get_clipboard_items', {
        limit: 1000,
        offset: 0,
        search: this.searchQuery || null,
        contentType: this.filterType === 'all' ? null : this.filterType,
      });
      this.items = items;
    } catch (error) {
      console.error('Failed to load items:', error);
    }
  }

  async deleteItem(id: number) {
    try {
      await invoke('delete_clipboard_item', { id });
      await this.loadItems();
    } catch (error) {
      console.error('Failed to delete item:', error);
    }
  }

  async togglePin(id: number) {
    try {
      await invoke('toggle_pin', { id });
      await this.loadItems();
    } catch (error) {
      console.error('Failed to toggle pin:', error);
    }
  }

  async pasteItem(id: number) {
    try {
      await invoke('paste_item', { id });
    } catch (error) {
      console.error('Failed to paste item:', error);
    }
  }

  setSearch(query: string) {
    this.searchQuery = query;
    this.loadItems();
  }

  setFilter(type: ContentType) {
    this.filterType = type;
    this.loadItems();
  }

  setSelected(id: number | null) {
    this.selectedId = id;
  }
}

export const clipboardStore = new ClipboardStore();
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager
npm run dev
```

预期输出：编译成功，无 TypeScript 错误

提交：
```bash
git add src/stores/ClipboardStore.ts
git commit -m "feat: create MobX store for clipboard state management"
```

---

### 任务 6.2：创建主应用组件

- [ ] 更新 `src/App.tsx`

```typescript
import { observer } from 'mobx-react-lite';
import { clipboardStore } from './stores/ClipboardStore';
import { useState } from 'react';

const App = observer(() => {
  const [searchInput, setSearchInput] = useState('');

  const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchInput(e.target.value);
    clipboardStore.setSearch(e.target.value);
  };

  const handleFilterChange = (type: any) => {
    clipboardStore.setFilter(type);
  };

  const handleItemClick = async (id: number) => {
    await clipboardStore.pasteItem(id);
  };

  const handleDelete = async (id: number, e: React.MouseEvent) => {
    e.stopPropagation();
    await clipboardStore.deleteItem(id);
  };

  const handleTogglePin = async (id: number, e: React.MouseEvent) => {
    e.stopPropagation();
    await clipboardStore.togglePin(id);
  };

  return (
    <div className="h-screen bg-gray-100 flex flex-col">
      <div className="p-4 bg-white shadow">
        <input
          type="text"
          placeholder="搜索剪切板..."
          value={searchInput}
          onChange={handleSearch}
          className="w-full px-4 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
      </div>

      <div className="flex gap-2 p-4 bg-white border-b">
        {['all', 'text', 'image', 'file_path'].map(type => (
          <button
            key={type}
            onClick={() => handleFilterChange(type)}
            className={`px-4 py-2 rounded ${
              clipboardStore.filterType === type
                ? 'bg-blue-500 text-white'
                : 'bg-gray-200 text-gray-700'
            }`}
          >
            {type === 'all' ? '全部' : type === 'text' ? '文本' : type === 'image' ? '图片' : '文件'}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {clipboardStore.filteredItems.map(item => (
          <div
            key={item.id}
            onClick={() => handleItemClick(item.id)}
            className="bg-white p-4 mb-2 rounded-lg shadow hover:shadow-md cursor-pointer transition"
          >
            <div className="flex justify-between items-start">
              <div className="flex-1">
                <p className="text-sm text-gray-600 mb-1">
                  {new Date(item.createdAt).toLocaleString()}
                </p>
                <p className="text-gray-800 break-words">{item.preview}</p>
              </div>
              <div className="flex gap-2 ml-4">
                <button
                  onClick={(e) => handleTogglePin(item.id, e)}
                  className={`px-2 py-1 rounded ${
                    item.pinned ? 'bg-yellow-400' : 'bg-gray-200'
                  }`}
                >
                  📌
                </button>
                <button
                  onClick={(e) => handleDelete(item.id, e)}
                  className="px-2 py-1 bg-red-500 text-white rounded hover:bg-red-600"
                >
                  🗑️
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
});

export default App;
```

测试命令：
```bash
cd /Users/real/cvte/clipboard-manager
npm run tauri dev
```

预期输出：应用显示剪切板列表，可以搜索、筛选、删除、固定

提交：
```bash
git add src/App.tsx
git commit -m "feat: create main UI with list, search, and filters"
```

---

## 阶段 7：完善和测试

### 任务 7.1：端到端测试

- [ ] 测试完整流程

测试步骤：
1. 启动应用：`npm run tauri dev`
2. 复制一段文本，验证是否出现在列表中
3. 使用搜索框搜索内容
4. 点击筛选按钮切换类型
5. 点击固定按钮，验证项目置顶
6. 双击列表项，验证内容粘贴到剪切板
7. 点击删除按钮，验证项目被删除
8. 按 Cmd+Shift+V 验证窗口显示/隐藏

预期输出：所有功能正常工作

提交：
```bash
git add .
git commit -m "test: verify end-to-end functionality"
```

---

## 总结

实施计划完成后，你将拥有一个功能完整的跨平台剪切板管理工具，包括：

✅ Tauri 2.x + React + TypeScript 项目结构
✅ SQLite 数据库存储
✅ 跨平台剪切板监听（macOS + Windows）
✅ 全局快捷键支持
✅ 响应式 UI（搜索、筛选、固定、删除）
✅ MobX 状态管理

**下一步扩展**（可选）：
- 图片预览优化
- 富文本支持
- P2P 同步
- WebDAV 同步
- 性能优化

