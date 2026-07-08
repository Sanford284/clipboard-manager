# 剪切板管理器 UI 重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把剪切板列表重构为紧凑单行高密度视图，替换 emoji 为类型化线性图标，加入手动深色模式、完整键盘流、图片抓取+缩略图、以及扩展的设置窗口。

**Architecture:** 前端组件化（拆分 App.tsx 为 SearchBar/FilterChips/ClipboardList/ClipboardRow/EmptyState + Icon + hooks + lib），主题用 Tailwind v4 class 模式 + 语义 CSS 变量。后端复用现有 KV 设置表新增 key，新增命令 `get_settings/set_setting/clear_history/set_autostart`，监控增加图片捕获与来源应用捕获，存储新增 `thumb_content` 列与历史清理。

**Tech Stack:** Tauri 2 (Rust) · React 19 · TypeScript · Vite · Tailwind v4 (`@tailwindcss/postcss`) · MobX 6 / mobx-react-lite 4 · `arboard` 3 · 新增 Rust `image` crate、`tauri-plugin-autostart`。

**Verification strategy（本项目无测试框架）:**
- 前端：`npx tsc --noEmit` 做类型检查；最终 `npm run build`。
- Rust 纯逻辑：`cargo test --manifest-path src-tauri/Cargo.toml`（仅 storage 层加单元测试）。
- 平台/集成：`npm run tauri dev` 手动验证（见每任务“手动验证”与最终 UAT）。
- 每个任务结束 `git commit`。

**Spec:** `docs/superpowers/specs/2026-07-08-clipboard-ui-refactor-design.md`

**Dependency note:** 共享文件（lib.rs / commands.rs / ClipboardStore.ts / App.tsx / styles.css）会被多个任务触及，**严格按任务编号顺序执行**以避免冲突。

---

## Phase A — 后端存储与命令

### Task 1: 存储层 — `thumb_content` 迁移、按 id 查询、历史清理

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/storage/models.rs`
- Modify: `src-tauri/src/storage/mod.rs`

- [ ] **Step 1: 加 `image` 依赖**

编辑 `src-tauri/Cargo.toml`，在 `[dependencies]` 末尾（`enigo = "0.2"` 之后）追加：

```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
```

- [ ] **Step 2: models.rs 增加 `thumb_content` 字段**

`src-tauri/src/storage/models.rs` 在 `blob_content` 字段后加 `thumb_content`：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardItem {
    pub id: i64,
    pub content_type: String,
    pub text_content: Option<String>,
    pub html_content: Option<String>,
    pub blob_content: Option<Vec<u8>>,
    pub thumb_content: Option<Vec<u8>>,
    pub file_path: Option<String>,
    pub preview: String,
    pub app_source: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub hash: String,
}
```

- [ ] **Step 3: mod.rs — 迁移 + 调整 insert/get_items + 新增方法**

在 `src-tauri/src/storage/mod.rs` 中：

(a) 在 `init_tables` 的 `Ok(())` 之前加迁移调用：

```rust
        Self::ensure_column(&conn, "clipboard_items", "thumb_content", "BLOB")?;

        Ok(())
```

(b) 在 `init_tables` 之后新增辅助方法（**关联函数，接收已持锁的 `&Connection`，避免重复加锁死锁**）：

```rust
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
```

(c) 把 `insert_item` 的 INSERT 替换为（增加 `thumb_content` 列与参数）：

```rust
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
```

(d) 把 `get_items` 的 SELECT 列表与映射改为（**移除 blob_content，新增 thumb_content**）：

```rust
        let mut query = "SELECT id, content_type, text_content, html_content, thumb_content, file_path, preview, app_source, pinned, created_at, hash FROM clipboard_items WHERE 1=1".to_string();
```

```rust
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
```

(e) 在 `toggle_pin` 之后、`get_setting` 之前新增三个方法：

```rust
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
```

(f) 在 `compute_hash` 之后新增字节哈希（图片去重用）：

```rust
    pub fn compute_hash_bytes(bytes: &[u8]) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }
```

- [ ] **Step 4: 加单元测试**

在 `src-tauri/src/storage/mod.rs` 文件**最末尾**（最后一个 `}` 之后）追加：

```rust
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
```

- [ ] **Step 5: 验证**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 全部通过（6 个 storage::tests）。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/storage/
git commit -m "feat(storage): add thumb_content column, get_item_by_id, history cleanup"
```

---

### Task 2: 新增设置/历史/自启动命令 + autostart 插件

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: 加 autostart 依赖**

`src-tauri/Cargo.toml` `[dependencies]` 追加：

```toml
tauri-plugin-autostart = "2"
```

- [ ] **Step 2: commands.rs 新增四个命令**

在 `src-tauri/src/commands.rs` 顶部 `use` 区追加：

```rust
use std::collections::HashMap;
```

在 `toggle_monitoring` 之后追加：

```rust
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
    key: String,
    value: String,
) -> Result<(), String> {
    let db = db.lock().unwrap();
    db.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_history(
    db: State<Arc<Mutex<Database>>>,
) -> Result<(), String> {
    let db = db.lock().unwrap();
    db.clear_history().map_err(|e| e.to_string())
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
```

- [ ] **Step 3: 注册 autostart 插件 + 命令**

`src-tauri/src/lib.rs`：

(a) 在 `.plugin(tauri_plugin_global_shortcut::Builder::new().build())` 之后追加：

```rust
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
```

(b) 把 `generate_handler![...]` 替换为：

```rust
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
```

- [ ] **Step 4: capabilities 加 autostart 权限**

`src-tauri/capabilities/default.json` 的 `permissions` 数组追加 `"autostart:default"`，结果为：

```json
  "permissions": [
    "core:default",
    "opener:default",
    "global-shortcut:allow-register",
    "global-shortcut:allow-unregister",
    "global-shortcut:allow-is-registered",
    "autostart:default"
  ]
```

- [ ] **Step 5: 验证编译**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功（新命令、插件、权限齐全）。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(backend): settings/history/autostart commands + autostart plugin"
```

### Task 3: 重构 `paste_item` —— 按 id 取条目 + 支持图片粘贴

**Files:**
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: 重写 `paste_item`**

把 `src-tauri/src/commands.rs` 中整个 `paste_item` 函数替换为：

```rust
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
                colors: img.into_raw(),
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

    // 隐藏窗口
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
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
```

- [ ] **Step 2: 验证编译**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "refactor(paste): use get_item_by_id, support image paste"
```

---

### Task 4: 来源应用捕获 —— 填充 `app_source`

**Files:**
- Modify: `src-tauri/Cargo.toml` (Windows 特性)
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Windows 增加需要的 crate 特性**

`src-tauri/Cargo.toml` 中 `[target.'cfg(target_os = "windows")'.dependencies]` 的 `windows` 特性数组追加 `"Win32_System_Threading"`：

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.52", features = [
    "Win32_Foundation",
    "Win32_System_DataExchange",
    "Win32_System_Threading",
    "Win32_UI_WindowsAndMessaging",
] }
```

- [ ] **Step 2: lib.rs 增加 `get_frontmost_app_name`**

在 `src-tauri/src/lib.rs` 现有 `get_frontmost_app_bundle_id` 函数之后追加（macOS + Windows 实现）：

```rust
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
    use windows::core::PCWSTR;
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
        QueryFullProcessImageNameW(handle, false, pwstr, &mut len).ok()?;
        let _ = CloseHandle(handle);
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        std::path::Path::new(&path)
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }
}
```

> 注：`PCWSTR` import 在 Windows 实现中未直接使用（用 PWSTR），可保留或删除；若编译告警未使用，删除该 `use` 行。

- [ ] **Step 3: 捕获闭包中填充 `app_source`**

`src-tauri/src/lib.rs` 的 `monitor.start(...)` 闭包内，在 `let item = match content {` 之前加：

```rust
                let app_source = get_frontmost_app_name();
```

然后把 Text 分支的 `app_source: None,` 改为 `app_source,`。

（Image 分支的 `app_source` 在 Task 5 一并改。）

- [ ] **Step 4: 验证**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功。

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(capture): populate app_source with frontmost app name"
```

---

### Task 5: 图片抓取 —— 监控检测图片 + 捕获闭包生成 blob/缩略图

**Files:**
- Modify: `src-tauri/src/clipboard/mod.rs`
- Modify: `src-tauri/src/clipboard/macos.rs`
- Modify: `src-tauri/src/clipboard/windows.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: enum 改为携带 `arboard::ImageData`**

`src-tauri/src/clipboard/mod.rs`：在 `use serde...` 之前加 `use arboard::ImageData;`，并把枚举改为：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipboardContent {
    Text(String),
    RichText { plain: String, html: String },
    Image(ImageData),
    FilePath(String),
}
```

> `ImageData` 需实现 `Send + Sync`（arboard 的 ImageData 是 `{width: usize, height: usize, colors: Vec<u8>}`，天然 Send/Sync）。若编译报 trait bound，给枚举加 `#[allow(dead_code)]` 无关；如仍报 Serialize/Deserialize，给 ImageData 加 `#[serde(skip)]`（见 Step 2 备注）。

- [ ] **Step 2: macos 监控加入图片检测**

`src-tauri/src/clipboard/macos.rs`：把 `thread::spawn` 内的循环体替换为：

```rust
            let mut last_text = clipboard.get_text().unwrap_or_default();
            let mut last_image: Vec<u8> = Vec::new();

            while running.load(Ordering::SeqCst) {
                if !paused.load(Ordering::SeqCst) {
                    let emitted_image = if let Ok(img) = clipboard.get_image() {
                        let sig = image_signature(&img);
                        if sig != last_image {
                            last_image = sig;
                            last_text.clear();
                            callback(ClipboardContent::Image(img));
                            true
                        } else { false }
                    } else { false };

                    if !emitted_image {
                        if let Ok(text) = clipboard.get_text() {
                            if !text.is_empty() && text != last_text {
                                last_text = text.clone();
                                last_image.clear();
                                callback(ClipboardContent::Text(text));
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(1500));
            }
```

并在文件末尾（`impl MacOSClipboardMonitor` 之前或文件底部）加辅助函数：

```rust
/// 用“宽高 + 前 64 字节”做轻量指纹，避免逐字节比较大图
fn image_signature(img: &arboard::ImageData) -> Vec<u8> {
    let mut sig = Vec::with_capacity(72);
    sig.extend_from_slice(&(img.width as u64).to_le_bytes());
    sig.extend_from_slice(&(img.height as u64).to_le_bytes());
    sig.extend_from_slice(&img.colors[..img.colors.len().min(64)]);
    sig
}
```

- [ ] **Step 3: windows 监控同样改造**

`src-tauri/src/clipboard/windows.rs`：把循环体替换为与 Step 2 相同的逻辑（`Duration::from_secs(1)`），并在文件底部加同样的 `image_signature` 函数。

- [ ] **Step 4: lib.rs Image 分支生成 blob + 缩略图 + 历史清理**

`src-tauri/src/lib.rs`：

(a) 在文件顶部（`use` 区之后、`run` 之前）加常量与辅助函数：

```rust
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
    let _ = image::DynamicImage::ImageRgb8(rgb)
        .write_to(&mut buf, image::ImageFormat::Jpeg);
    buf.into_inner()
}
```

(b) 把捕获闭包里的 `ClipboardContent::Image(data) => { ... }` 分支替换为：

```rust
                    ClipboardContent::Image(img) => {
                        // 由原始 RGBA 构图
                        let rgba = match image::RgbaImage::from_raw(
                            img.width as u32, img.height as u32, img.colors.clone(),
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
```

> 注：`app_source` 变量在 Task 4 Step 3 已在闭包顶部声明。若未声明，先按 Task 4 加 `let app_source = get_frontmost_app_name();`。

(c) 在闭包里 `if let Ok(id) = db.insert_item(&item) { ... }` 之后追加历史清理（保留 `emit`）：

```rust
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
```

- [ ] **Step 5: 验证**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功。

手动验证（macOS）：`npm run tauri dev` → 复制一张截图 → 主窗口出现粉色图片行 + 缩略图；点击该行 → 图片粘贴到目标 app。
> 若 `ImageData` Serialize 报错：把 enum 的 `Image(ImageData)` 改为 `Image(#[serde(skip)] ImageData)`（捕获路径不依赖序列化）。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/clipboard/ src-tauri/src/lib.rs
git commit -m "feat(capture): image capture with png blob + jpeg thumbnail"
```

## Phase B — 前端基础（主题、格式、图标）

### Task 6: 主题系统 —— Tailwind v4 dark 变体 + 语义变量 + theme.ts

**Files:**
- Modify: `src/styles.css`
- Create: `src/lib/theme.ts`

- [ ] **Step 1: 重写 styles.css**

把 `src/styles.css` 全部内容替换为：

```css
@import "tailwindcss";

@custom-variant dark (&:where(.dark, .dark *));

:root {
  --bg: #f4f5f7;
  --surface: #ffffff;
  --surface-hover: #f7f8fa;
  --surface-active: #eef4ff;
  --text: #1f2937;
  --muted: #9ca3af;
  --border: #eceef1;
  --accent: #3b82f6;
  --accent-soft: #eff6ff;
}

.dark {
  --bg: #1a1b1e;
  --surface: #24262b;
  --surface-hover: #2c2f35;
  --surface-active: #1e2a44;
  --text: #e5e7eb;
  --muted: #7b8088;
  --border: #33363c;
  --accent: #3b82f6;
  --accent-soft: #1e2a44;
}

@theme inline {
  --color-bg: var(--bg);
  --color-surface: var(--surface);
  --color-surface-hover: var(--surface-hover);
  --color-surface-active: var(--surface-active);
  --color-text: var(--text);
  --color-muted: var(--muted);
  --color-border: var(--border);
  --color-accent: var(--accent);
  --color-accent-soft: var(--accent-soft);
}

html, body, #root {
  height: 100%;
  margin: 0;
  background: var(--bg);
  color: var(--text);
  font-family: -apple-system, "PingFang SC", "Segoe UI", "Microsoft YaHei", sans-serif;
}

/* 滚动条随主题 */
* {
  scrollbar-width: thin;
  scrollbar-color: var(--border) transparent;
}
*::-webkit-scrollbar { width: 8px; height: 8px; }
*::-webkit-scrollbar-thumb { background: var(--border); border-radius: 4px; }
```

- [ ] **Step 2: 创建 theme.ts**

`src/lib/theme.ts`：

```ts
export type Theme = 'light' | 'dark';

export function applyTheme(theme: Theme): void {
  const root = document.documentElement;
  if (theme === 'dark') root.classList.add('dark');
  else root.classList.remove('dark');
}
```

- [ ] **Step 3: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: Commit**

```bash
git add src/styles.css src/lib/theme.ts
git commit -m "feat(ui): semantic theme tokens + dark variant + theme helper"
```

---

### Task 7: 格式工具 + 类型图标组件

**Files:**
- Create: `src/lib/format.ts`
- Create: `src/components/Icon.tsx`

- [ ] **Step 1: 创建 format.ts**

`src/lib/format.ts`：

```ts
import type { ClipboardItem } from '../stores/ClipboardStore';

export type RowKind = 'text' | 'link' | 'code' | 'file' | 'image';

const URL_RE = /^https?:\/\/\S+$/i;
const CODE_RE = /(\bfn\b|\bfunction\b|=>|;\s*$|^\s*\{|\bconst\b|\blet\b|\bimport\b|\bpub fn\b)/m;

export function classify(item: ClipboardItem): RowKind {
  if (item.content_type === 'image') return 'image';
  if (item.content_type === 'file_path') return 'file';
  const text = (item.text_content ?? '').trim();
  if (URL_RE.test(text)) return 'link';
  if (text.includes('\n') && CODE_RE.test(text)) return 'code';
  return 'text';
}

export function isUrl(s: string): boolean {
  return URL_RE.test(s.trim());
}

function pad(n: number): string {
  return n < 10 ? '0' + n : String(n);
}

export function formatTime(ts: number): string {
  const now = Date.now();
  const diff = now - ts;
  const d = new Date(ts);
  const min = 60_000;
  const hour = 3_600_000;
  const day = 86_400_000;
  if (diff < min) return '刚刚';
  if (diff < hour) return `${Math.floor(diff / min)}分钟前`;
  const today = new Date();
  const hhmm = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  if (d.toDateString() === today.toDateString()) return hhmm;
  const yesterday = new Date(today.getTime() - day);
  if (d.toDateString() === yesterday.toDateString()) return `昨天 ${hhmm}`;
  return `${d.getMonth() + 1}月${d.getDate()}日`;
}
```

- [ ] **Step 2: 创建 Icon.tsx**

`src/components/Icon.tsx`：

```tsx
import type { RowKind } from '../lib/format';

interface KindStyle {
  cls: string;
  paths: string[];
}

const KIND: Record<RowKind, KindStyle> = {
  text: {
    cls: 'bg-[#eef2ff] text-[#4f46e5] dark:bg-[#312e81] dark:text-[#a5b4fc]',
    paths: ['M5 6h14', 'M5 12h14', 'M5 18h9'],
  },
  link: {
    cls: 'bg-[#ecfdf5] text-[#059669] dark:bg-[#064e3b] dark:text-[#6ee7b7]',
    paths: [
      'M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1',
      'M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1',
    ],
  },
  code: {
    cls: 'bg-[#fff7ed] text-[#ea580c] dark:bg-[#7c2d12] dark:text-[#fdba74]',
    paths: ['m8 8-4 4 4 4', 'm16 8 4 4-4 4'],
  },
  file: {
    cls: 'bg-[#eff6ff] text-[#2563eb] dark:bg-[#1e3a8a] dark:text-[#93c5fd]',
    paths: ['M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z', 'M14 3v5h5'],
  },
  image: {
    cls: 'bg-[#fdf2f8] text-[#db2777] dark:bg-[#831843] dark:text-[#f9a8d4]',
    paths: ['M3 5h18v14H3z', 'm21 16-5-5-9 9'],
  },
};

export function Icon({ kind, className = '' }: { kind: RowKind; className?: string }) {
  const s = KIND[kind];
  return (
    <span
      className={`inline-flex items-center justify-center w-[18px] h-[18px] rounded-[5px] shrink-0 ${s.cls} ${className}`}
      aria-hidden
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round">
        {s.paths.map((d, i) => (
          <path key={i} d={d} />
        ))}
      </svg>
    </span>
  );
}

export function SearchIcon({ className = '' }: { className?: string }) {
  return (
    <svg className={className} width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round">
      <circle cx="11" cy="11" r="7" />
      <path d="m21 21-4.3-4.3" />
    </svg>
  );
}

export function PinIcon({ active = false, className = '' }: { active?: boolean; className?: string }) {
  return (
    <svg
      className={className}
      width="13" height="13" viewBox="0 0 24 24"
      fill={active ? 'currentColor' : 'none'}
      stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round"
    >
      <path d="M12 17v5" />
      <path d="M9 3h6l-1 6 4 3v2H6v-2l4-3-1-6z" />
    </svg>
  );
}

export function TrashIcon({ className = '' }: { className?: string }) {
  return (
    <svg
      className={className}
      width="13" height="13" viewBox="0 0 24 24"
      fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round"
    >
      <path d="M4 7h16" />
      <path d="M10 11v6M14 11v6" />
      <path d="M6 7l1 13h10l1-13" />
      <path d="M9 7V4h6v3" />
    </svg>
  );
}
```

- [ ] **Step 3: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: Commit**

```bash
git add src/lib/format.ts src/components/Icon.tsx
git commit -m "feat(ui): format/classify helpers + typed line icons"
```

## Phase C — Store 与组件

### Task 8: ClipboardStore —— 设置状态、选中导航、link 过滤

**Files:**
- Modify: `src/stores/ClipboardStore.ts`

- [ ] **Step 1: 扩展接口与 store**

把 `src/stores/ClipboardStore.ts` 全部替换为：

```ts
import { makeAutoObservable } from 'mobx';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { isUrl } from '../lib/format';

export interface ClipboardItem {
  id: number;
  content_type: string;
  text_content?: string;
  html_content?: string;
  blob_content?: number[];
  thumb_content?: number[];
  file_path?: string;
  preview: string;
  app_source?: string;
  pinned: boolean;
  created_at: number;
  hash: string;
}

export type ContentType = 'all' | 'text' | 'link' | 'image' | 'file_path';

export interface Settings {
  theme: 'light' | 'dark';
  show_source: boolean;
  window_width: number;
  window_height: number;
  history_mode: 'auto' | 'never' | 'manual';
  history_limit: number;
}

const DEFAULT_SETTINGS: Settings = {
  theme: 'light',
  show_source: true,
  window_width: 800,
  window_height: 600,
  history_mode: 'never',
  history_limit: 500,
};

class ClipboardStore {
  items: ClipboardItem[] = [];
  searchQuery: string = '';
  filterType: ContentType = 'all';
  selectedId: number | null = null;
  settings: Settings = DEFAULT_SETTINGS;

  constructor() {
    makeAutoObservable(this);
    this.init();
  }

  async init() {
    await this.loadSettings();
    await this.loadItems();
    listen('clipboard-changed', () => {
      this.loadItems();
    });
  }

  get filteredItems(): ClipboardItem[] {
    return this.items
      .filter((item) => {
        if (this.filterType === 'all') return true;
        if (this.filterType === 'link') {
          if (item.content_type !== 'text') return false;
          return isUrl(item.text_content ?? '');
        }
        if (item.content_type !== this.filterType) return false;
        return true;
      })
      .filter((item) => {
        if (!this.searchQuery) return true;
        return item.preview.toLowerCase().includes(this.searchQuery.toLowerCase());
      })
      .sort((a, b) => {
        if (a.pinned !== b.pinned) return b.pinned ? 1 : -1;
        return b.created_at - a.created_at;
      });
  }

  get selectedItem(): ClipboardItem | undefined {
    return this.filteredItems.find((i) => i.id === this.selectedId);
  }

  async loadSettings() {
    try {
      const raw = await invoke<Record<string, string>>('get_settings');
      this.settings = {
        theme: (raw.theme as 'light' | 'dark') ?? DEFAULT_SETTINGS.theme,
        show_source: raw.show_source === undefined ? DEFAULT_SETTINGS.show_source : raw.show_source === 'true',
        window_width: num(raw.window_width, DEFAULT_SETTINGS.window_width, 320, 1200),
        window_height: num(raw.window_height, DEFAULT_SETTINGS.window_height, 400, 1400),
        history_mode: (raw.history_mode as Settings['history_mode']) ?? DEFAULT_SETTINGS.history_mode,
        history_limit: num(raw.history_limit, DEFAULT_SETTINGS.history_limit, 50, 100000),
      };
    } catch (e) {
      console.error('Failed to load settings:', e);
    }
  }

  async loadItems() {
    try {
      const contentType =
        this.filterType === 'all' || this.filterType === 'link' ? null : this.filterType;
      const items = await invoke<ClipboardItem[]>('get_clipboard_items', {
        limit: 1000,
        offset: 0,
        search: this.searchQuery || null,
        contentType,
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

  async saveSetting(key: string, value: string) {
    try {
      await invoke('set_setting', { key, value });
      await this.loadSettings();
    } catch (e) {
      console.error('Failed to save setting:', e);
    }
  }

  setSearch(query: string) {
    this.searchQuery = query;
    this.selectedId = this.filteredItems[0]?.id ?? null;
    this.loadItems();
  }

  setFilter(type: ContentType) {
    this.filterType = type;
    this.selectedId = this.filteredItems[0]?.id ?? null;
    this.loadItems();
  }

  setSelected(id: number | null) {
    this.selectedId = id;
  }

  /** 键盘导航：delta = +1 下 / -1 上 */
  moveSelection(delta: number): ClipboardItem | undefined {
    const items = this.filteredItems;
    if (!items.length) return undefined;
    const idx = items.findIndex((i) => i.id === this.selectedId);
    const base = idx < 0 ? (delta > 0 ? -1 : items.length) : idx;
    const next = Math.min(items.length - 1, Math.max(0, base + delta));
    this.selectedId = items[next].id;
    return items[next];
  }

  itemAt(n: number): ClipboardItem | undefined {
    return this.filteredItems[n - 1];
  }
}

function num(v: string | undefined, dflt: number, min: number, max: number): number {
  if (v === undefined) return dflt;
  const n = parseInt(v, 10);
  if (Number.isNaN(n)) return dflt;
  return Math.min(max, Math.max(min, n));
}

export const clipboardStore = new ClipboardStore();
```

- [ ] **Step 2: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 3: Commit**

```bash
git add src/stores/ClipboardStore.ts
git commit -m "feat(store): settings state, selection navigation, link filter"
```

---

### Task 9: 小组件 —— SearchBar / FilterChips / EmptyState

**Files:**
- Create: `src/components/SearchBar.tsx`
- Create: `src/components/FilterChips.tsx`
- Create: `src/components/EmptyState.tsx`

- [ ] **Step 1: SearchBar.tsx**

```tsx
import { forwardRef } from 'react';
import { observer } from 'mobx-react-lite';
import { clipboardStore } from '../stores/ClipboardStore';
import { SearchIcon } from './Icon';

interface Props {
  onCmdF?: () => void;
}

export const SearchBar = observer(forwardRef<HTMLInputElement, Props>((_props, ref) => {
  return (
    <div className="flex items-center gap-2 px-3 h-11 border-b border-border bg-surface">
      <SearchIcon className="text-muted shrink-0" />
      <input
        ref={ref}
        type="text"
        placeholder="搜索剪切板…"
        value={clipboardStore.searchQuery}
        onChange={(e) => clipboardStore.setSearch(e.target.value)}
        className="flex-1 min-w-0 bg-transparent outline-none text-text placeholder:text-muted text-[13px]"
      />
      <kbd className="text-[11px] text-muted shrink-0 border border-border rounded px-1.5 py-0.5">
        ⌘F
      </kbd>
    </div>
  );
}));
```

- [ ] **Step 2: FilterChips.tsx**

```tsx
import { observer } from 'mobx-react-lite';
import { clipboardStore, type ContentType } from '../stores/ClipboardStore';

const CHIPS: { key: ContentType; label: string }[] = [
  { key: 'all', label: '全部' },
  { key: 'text', label: '文本' },
  { key: 'link', label: '链接' },
  { key: 'image', label: '图片' },
  { key: 'file_path', label: '文件' },
];

export const FilterChips = observer(() => {
  return (
    <div className="flex items-center gap-1.5 px-3 h-9 border-b border-border bg-surface">
      {CHIPS.map((c) => {
        const active = clipboardStore.filterType === c.key;
        return (
          <button
            key={c.key}
            onClick={() => clipboardStore.setFilter(c.key)}
            className={`text-[12px] px-2.5 py-0.5 rounded-full shrink-0 transition-colors ${
              active
                ? 'bg-accent text-white'
                : 'bg-transparent text-muted hover:bg-surface-hover'
            }`}
          >
            {c.label}
          </button>
        );
      })}
    </div>
  );
});
```

- [ ] **Step 3: EmptyState.tsx**

```tsx
import { observer } from 'mobx-react-lite';
import { clipboardStore } from '../stores/ClipboardStore';

export const EmptyState = observer(() => {
  const searching = clipboardStore.searchQuery.length > 0;
  return (
    <div className="flex flex-col items-center justify-center h-full text-muted gap-2 py-16">
      <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5}>
        <rect x="6" y="4" width="12" height="16" rx="2" />
        <path d="M9 8h6M9 12h6M9 16h3" strokeLinecap="round" />
      </svg>
      <p className="text-[13px]">
        {searching ? `没有匹配「${clipboardStore.searchQuery}」的记录` : '还没有剪切板记录，复制点什么吧'}
      </p>
    </div>
  );
});
```

- [ ] **Step 4: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 5: Commit**

```bash
git add src/components/SearchBar.tsx src/components/FilterChips.tsx src/components/EmptyState.tsx
git commit -m "feat(ui): SearchBar, FilterChips, EmptyState components"
```

### Task 10: ClipboardRow —— 单行（图标/缩略图 + 占满预览 + 来源·时间 + hover 操作）

**Files:**
- Create: `src/components/ClipboardRow.tsx`

- [ ] **Step 1: ClipboardRow.tsx**

```tsx
import { useEffect, useMemo } from 'react';
import { observer } from 'mobx-react-lite';
import type { ClipboardItem } from '../stores/ClipboardStore';
import { classify, formatTime } from '../lib/format';
import { Icon, PinIcon, TrashIcon } from './Icon';

interface Props {
  item: ClipboardItem;
  selected: boolean;
  showSource: boolean;
  onPaste: () => void;
  onPin: (e: React.MouseEvent) => void;
  onDelete: (e: React.MouseEvent) => void;
}

export const ClipboardRow = observer(({ item, selected, showSource, onPaste, onPin, onDelete }: Props) => {
  const kind = classify(item);

  const thumbUrl = useMemo(() => {
    if (kind !== 'image' || !item.thumb_content?.length) return null;
    const blob = new Blob([new Uint8Array(item.thumb_content)], { type: 'image/jpeg' });
    return URL.createObjectURL(blob);
  }, [kind, item.thumb_content]);

  useEffect(() => {
    return () => {
      if (thumbUrl) URL.revokeObjectURL(thumbUrl);
    };
  }, [thumbUrl]);

  const time = formatTime(item.created_at);
  const sourceTime = showSource && item.app_source ? `${item.app_source} · ${time}` : time;

  return (
    <div
      onClick={onPaste}
      data-id={item.id}
      className={`group flex items-center gap-2.5 h-[30px] px-3 cursor-pointer border-b border-border text-text text-[13px] ${
        selected ? 'bg-accent-soft' : 'hover:bg-surface-hover'
      }`}
    >
      {thumbUrl ? (
        <img src={thumbUrl} alt="" className="w-[18px] h-[18px] rounded-[5px] object-cover shrink-0" />
      ) : (
        <Icon kind={kind} />
      )}

      <span className="flex-1 min-w-0 truncate">{item.preview}</span>

      <span className="shrink-0 text-[11px] text-muted">{sourceTime}</span>

      <div className="flex items-center gap-1 shrink-0">
        <button
          onClick={onPin}
          title="置顶"
          className={`p-0.5 rounded transition-opacity ${
            item.pinned
              ? 'text-yellow-500 opacity-100'
              : 'text-muted hover:text-text opacity-0 group-hover:opacity-100'
          }`}
        >
          <PinIcon active={item.pinned} />
        </button>
        <button
          onClick={onDelete}
          title="删除"
          className="p-0.5 rounded text-muted hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity"
        >
          <TrashIcon />
        </button>
      </div>
    </div>
  );
});
```

- [ ] **Step 2: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 3: Commit**

```bash
git add src/components/ClipboardRow.tsx
git commit -m "feat(ui): dense ClipboardRow with icon/thumb, full-width preview, hover actions"
```

---

### Task 11: ClipboardList + 完整键盘流（useKeyboardNav）

**Files:**
- Create: `src/hooks/useKeyboardNav.ts`
- Create: `src/components/ClipboardList.tsx`

- [ ] **Step 1: useKeyboardNav.ts**

```ts
import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { clipboardStore } from '../stores/ClipboardStore';

interface Options {
  focusSearch: () => void;
}

/** 主窗口全局键盘流：↑↓ 选择、Enter 粘贴、1-9 快速粘贴、Esc、Cmd/Ctrl+F */
export function useKeyboardNav({ focusSearch }: Options) {
  useEffect(() => {
    const handler = async (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;

      // Cmd/Ctrl + F —— 聚焦搜索
      if (mod && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        focusSearch();
        return;
      }

      if (e.key === 'Escape') {
        if (clipboardStore.searchQuery) {
          clipboardStore.setSearch('');
        } else {
          await getCurrentWindow().hide();
        }
        return;
      }

      // 数字 1-9：快速粘贴第 N 条（无修饰键）
      if (!mod && /^[1-9]$/.test(e.key)) {
        const n = parseInt(e.key, 10);
        const item = clipboardStore.itemAt(n);
        if (item) {
          e.preventDefault();
          await clipboardStore.pasteItem(item.id);
        }
        return;
      }

      if (e.key === 'ArrowDown') {
        e.preventDefault();
        clipboardStore.moveSelection(1);
        return;
      }

      if (e.key === 'ArrowUp') {
        e.preventDefault();
        clipboardStore.moveSelection(-1);
        return;
      }

      if (e.key === 'Enter') {
        e.preventDefault();
        const item = clipboardStore.selectedItem ?? clipboardStore.filteredItems[0];
        if (item) {
          await clipboardStore.pasteItem(item.id);
        }
        return;
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [focusSearch]);
}
```

- [ ] **Step 2: ClipboardList.tsx**

```tsx
import { useEffect, useRef } from 'react';
import { observer } from 'mobx-react-lite';
import { clipboardStore } from '../stores/ClipboardStore';
import { ClipboardRow } from './ClipboardRow';
import { EmptyState } from './EmptyState';

export const ClipboardList = observer(() => {
  const items = clipboardStore.filteredItems;
  const containerRef = useRef<HTMLDivElement>(null);

  // 选中项滚动入视口
  useEffect(() => {
    if (clipboardStore.selectedId == null) return;
    const el = containerRef.current?.querySelector(`[data-id="${clipboardStore.selectedId}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [clipboardStore.selectedId]);

  if (items.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto bg-bg">
        <EmptyState />
      </div>
    );
  }

  return (
    <div ref={containerRef} className="flex-1 overflow-y-auto bg-surface">
      {items.map((item) => (
        <ClipboardRow
          key={item.id}
          item={item}
          selected={item.id === clipboardStore.selectedId}
          showSource={clipboardStore.settings.show_source}
          onPaste={() => clipboardStore.pasteItem(item.id)}
          onPin={(e) => {
            e.stopPropagation();
            clipboardStore.togglePin(item.id);
          }}
          onDelete={(e) => {
            e.stopPropagation();
            clipboardStore.deleteItem(item.id);
          }}
        />
      ))}
    </div>
  );
});
```

- [ ] **Step 3: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: Commit**

```bash
git add src/hooks/useKeyboardNav.ts src/components/ClipboardList.tsx
git commit -m "feat(ui): ClipboardList + full keyboard navigation"
```

## Phase D — 集成

### Task 12: App.tsx —— 组合组件 + 主题 + 键盘流

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: 重写 App.tsx**

把 `src/App.tsx` 全部替换为：

```tsx
import { useEffect, useRef } from 'react';
import { observer } from 'mobx-react-lite';
import { clipboardStore } from './stores/ClipboardStore';
import { applyTheme } from './lib/theme';
import { useKeyboardNav } from './hooks/useKeyboardNav';
import { SearchBar } from './components/SearchBar';
import { FilterChips } from './components/FilterChips';
import { ClipboardList } from './components/ClipboardList';

const App = observer(() => {
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    applyTheme(clipboardStore.settings.theme);
  }, [clipboardStore.settings.theme]);

  useKeyboardNav({
    focusSearch: () => searchRef.current?.focus(),
  });

  return (
    <div className="h-screen flex flex-col bg-bg">
      <SearchBar ref={searchRef} />
      <FilterChips />
      <ClipboardList />
    </div>
  );
});

export default App;
```

- [ ] **Step 2: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 3: Commit**

```bash
git add src/App.tsx
git commit -m "feat(ui): compose main window shell with theme + keyboard nav"
```

---

### Task 13: Settings.tsx —— 升级为通用设置 + main.tsx 路由

**Files:**
- Create: `src/Settings.tsx`
- Modify: `src/main.tsx`

- [ ] **Step 1: 创建 Settings.tsx**

`src/Settings.tsx`（由原 `ShortcutSettings.tsx` 升级；保留快捷键录入逻辑，新增外观/行为分区）：

```tsx
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import './styles.css';

const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta']);

function fmt(shortcut: string): string {
  return shortcut
    .replace(/CommandOrControl/g, '⌘')
    .replace(/Control/g, '⌃')
    .replace(/Shift/g, '⇧')
    .replace(/Alt/g, '⌥')
    .replace(/\+/g, '');
}

export default function Settings() {
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [currentShortcut, setCurrentShortcut] = useState('');
  const [newShortcut, setNewShortcut] = useState('');
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    const s = await invoke<Record<string, string>>('get_settings');
    setSettings(s);
    setCurrentShortcut(s.shortcut ?? (await invoke<string>('get_shortcut')));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const save = (key: string, value: string) => {
    setSettings((p) => ({ ...p, [key]: value }));
    invoke('set_setting', { key, value });
  };

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();
    const modifiers: string[] = [];
    if (e.metaKey) modifiers.push('CommandOrControl');
    else if (e.ctrlKey) modifiers.push('Control');
    if (e.shiftKey) modifiers.push('Shift');
    if (e.altKey) modifiers.push('Alt');
    if (MODIFIER_KEYS.has(e.key)) return;
    if (modifiers.length === 0) {
      setError('快捷键必须包含修饰键（⌘/⌃/⇧/⌥）');
      return;
    }
    const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
    setNewShortcut([...modifiers, key].join('+'));
    setRecording(false);
    setError('');
  }, [recording]);

  const handleSaveShortcut = async () => {
    const sc = newShortcut || currentShortcut;
    try {
      await invoke('set_shortcut', { shortcut: sc });
      setCurrentShortcut(sc);
      setNewShortcut('');
    } catch (e) {
      setError(`无法注册快捷键: ${e}`);
    }
  };

  const close = async () => getCurrentWindow().close();

  const labelCls = 'block text-[12px] text-[var(--muted)] mb-1';

  return (
    <div className="h-screen bg-bg text-[var(--text)] overflow-y-auto">
      <div className="max-w-md mx-auto p-5 space-y-6">

        {/* 快捷键 */}
        <section className="space-y-2">
          <h2 className="text-[14px] font-semibold">快捷键</h2>
          <div>
            <label className={labelCls}>当前快捷键</label>
            <div className="text-[15px] font-mono">{fmt(currentShortcut)}</div>
          </div>
          <div>
            <label className={labelCls}>新快捷键</label>
            <div
              tabIndex={0}
              onKeyDown={handleKeyDown}
              onClick={() => { setRecording(true); setError(''); }}
              onBlur={() => setRecording(false)}
              className={`w-full px-3 py-2 border-2 rounded-lg text-center font-mono text-[15px] cursor-pointer select-none ${
                recording
                  ? 'border-[var(--accent)] bg-[var(--accent-soft)]'
                  : 'border-[var(--border)] bg-[var(--surface)]'
              }`}
            >
              {recording
                ? '按下新的快捷键组合…'
                : newShortcut
                  ? fmt(newShortcut)
                  : '点击此处录入快捷键'}
            </div>
          </div>
          {error && <p className="text-red-500 text-[12px]">{error}</p>}
          <div className="flex gap-2">
            <button onClick={handleSaveShortcut} disabled={!newShortcut}
              className="flex-1 px-3 py-1.5 bg-[var(--accent)] text-white rounded-lg disabled:opacity-50">
              保存快捷键
            </button>
            <button onClick={() => { setNewShortcut('CommandOrControl+Shift+V'); setError(''); }}
              className="px-3 py-1.5 bg-[var(--surface-hover)] rounded-lg">
              恢复默认
            </button>
          </div>
        </section>

        {/* 外观 */}
        <section className="space-y-3">
          <h2 className="text-[14px] font-semibold">外观</h2>
          <div>
            <label className={labelCls}>主题</label>
            <select
              value={settings.theme ?? 'light'}
              onChange={(e) => save('theme', e.target.value)}
              className="w-full px-2 py-1.5 bg-[var(--surface)] border border-[var(--border)] rounded-lg text-[13px]"
            >
              <option value="light">浅色</option>
              <option value="dark">深色</option>
            </select>
          </div>
          <div className="flex gap-3">
            <div className="flex-1">
              <label className={labelCls}>默认宽度 (px)</label>
              <input type="number" min={320} max={1200}
                value={settings.window_width ?? 800}
                onChange={(e) => save('window_width', e.target.value)}
                className="w-full px-2 py-1.5 bg-[var(--surface)] border border-[var(--border)] rounded-lg text-[13px]" />
            </div>
            <div className="flex-1">
              <label className={labelCls}>默认高度 (px)</label>
              <input type="number" min={400} max={1400}
                value={settings.window_height ?? 600}
                onChange={(e) => save('window_height', e.target.value)}
                className="w-full px-2 py-1.5 bg-[var(--surface)] border border-[var(--border)] rounded-lg text-[13px]" />
            </div>
          </div>
        </section>

        {/* 行为 */}
        <section className="space-y-3">
          <h2 className="text-[14px] font-semibold">行为</h2>
          <label className="flex items-center justify-between text-[13px]">
            <span>每行显示来源应用</span>
            <input type="checkbox"
              checked={settings.show_source === 'true'}
              onChange={(e) => save('show_source', String(e.target.checked))} />
          </label>

          <div>
            <label className={labelCls}>历史保留</label>
            <select
              value={settings.history_mode ?? 'never'}
              onChange={(e) => save('history_mode', e.target.value)}
              className="w-full px-2 py-1.5 bg-[var(--surface)] border border-[var(--border)] rounded-lg text-[13px]"
            >
              <option value="never">永不清除</option>
              <option value="auto">自动保留最近 N 条</option>
              <option value="manual">手动清除</option>
            </select>
          </div>
          {settings.history_mode === 'auto' && (
            <div>
              <label className={labelCls}>N（最近条数）</label>
              <input type="number" min={50} max={100000}
                value={settings.history_limit ?? 500}
                onChange={(e) => save('history_limit', e.target.value)}
                className="w-full px-2 py-1.5 bg-[var(--surface)] border border-[var(--border)] rounded-lg text-[13px]" />
            </div>
          )}
          {settings.history_mode === 'manual' && (
            <button
              onClick={async () => { await invoke('clear_history'); alert('已清空非置顶记录'); }}
              className="px-3 py-1.5 bg-red-500 text-white rounded-lg text-[13px]">
              立即清空
            </button>
          )}

          <label className="flex items-center justify-between text-[13px]">
            <span>开机自启动</span>
            <input type="checkbox"
              checked={settings.autostart === 'true'}
              onChange={async (e) => {
                save('autostart', String(e.target.checked));
                await invoke('set_autostart', { enabled: e.target.checked });
              }} />
          </label>
        </section>

        <div className="pt-2">
          <button onClick={close}
            className="w-full px-3 py-2 bg-[var(--surface-hover)] rounded-lg text-[13px]">
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: 删除旧文件 + 更新路由**

```bash
rm src/ShortcutSettings.tsx
```

把 `src/main.tsx` 全部替换为：

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import Settings from "./Settings";

const params = new URLSearchParams(window.location.search);
const windowType = params.get("window");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {windowType === "settings" ? <Settings /> : <App />}
  </React.StrictMode>,
);
```

- [ ] **Step 3: 验证**

Run: `npx tsc --noEmit`
Expected: 无错误。

- [ ] **Step 4: Commit**

```bash
git add src/Settings.tsx src/main.tsx src/ShortcutSettings.tsx
git commit -m "feat(settings): general settings window (shortcut/appearance/behavior)"
```

---

### Task 14: 主窗口默认尺寸 + 托盘设置入口 + 最终验证

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 启动时应用默认窗口尺寸**

`src-tauri/src/lib.rs` 的 `setup` 闭包中，在 `// --- System Tray ---` 之前插入（读取设置调整主窗口尺寸，仅在创建时一次）：

```rust
            // --- Apply default window size from settings ---
            if let Some(window) = app.get_webview_window("main") {
                let w = db_for_shortcut.lock().unwrap()
                    .get_setting("window_width").ok().flatten()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(800.0)
                    .clamp(320.0, 1200.0);
                let h = db_for_shortcut.lock().unwrap()
                    .get_setting("window_height").ok().flatten()
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(600.0)
                    .clamp(400.0, 1400.0);
                let _ = window.set_size(tauri::LogicalSize::new(w, h));
            }
```

> `db_for_shortcut` 已在 setup 上方声明为 `app.state::<Arc<Mutex<Database>>>()`。

- [ ] **Step 2: 托盘设置窗口标题/尺寸**

把 `"shortcut_settings"` 分支里 `WebviewWindowBuilder` 的 `.title("修改快捷键")` 改为 `.title("设置")`，`.inner_size(400.0, 320.0)` 改为 `.inner_size(560.0, 640.0)`，`.resizable(false)` 改为 `.resizable(true)`：

```rust
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
```

并把菜单项文案 `"修改快捷键..."` 改为 `"设置..."`：

```rust
            let shortcut_item = MenuItemBuilder::with_id("shortcut_settings", "设置...")
                .build(app)?;
```

- [ ] **Step 3: 编译 + 前端构建**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功。

Run: `npm run build`
Expected: `tsc` 通过、vite 构建成功。

- [ ] **Step 4: 手动 UAT（spec §13）**

Run: `npm run tauri dev`，逐项验证：

1. 视口内可见 ~15–18 条；预览占满整行、仅末尾省略。✅
2. 图标为线性上色方块，无 emoji。✅
3. 设置 → 切换深色/浅色，即时生效，两套配色可用。✅
4. 键盘：↑↓ 移动选中、Enter 粘贴、数字 1–9 快速粘贴、Esc（先清搜索再隐藏）、⌘F 聚焦搜索。✅
5. 复制一张截图 → 行内出现缩略图；点击该行 → 原图粘贴到目标 app。✅
6. 复制超 5MB 的图 → 不入库（无新行）。✅
7. 来源应用：每行时间位显示 `微信 · 刚刚`（开启 show_source 时）。✅
8. 历史保留设为「自动 N=5」→ 连续复制 8 段文字后，非置顶仅保留最近 5 条。✅
9. 历史保留设为「手动」→ 点「立即清空」→ 非置顶全清，置顶保留。✅
10. 改默认宽度为 1000 → 退出后重新启动 → 主窗口宽 1000。✅
11. 开机自启动开关切换无报错。✅

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(window): default window size from settings; tray settings entry"
```

---

## 收尾

- [ ] **Step 6: 全量验证**

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
```

全部通过即完成。

- [ ] **Step 7: 删除残留（可选）**

`src/App.css` 为 Vite 模板残留且未被任何文件 import，可删除：

```bash
git rm src/App.css
git commit -m "chore: remove unused Vite template App.css"
```

---

## 计划自检（spec 覆盖对照）

| Spec 条目 | 实现任务 |
|---|---|
| §4 主窗口布局/单行/占满预览/时间/来源/空状态 | Task 9, 10, 11, 12 |
| §4.5 过滤芯片（含 link） | Task 8 (store), 9 |
| §5 图标系统 | Task 7 |
| §6 图片抓取+缩略图+尺寸上限+迁移 | Task 1 (迁移), 5 |
| §6.4 粘贴图片 | Task 3 |
| §7 主题手动切换 | Task 6, 12 |
| §8 键盘流 | Task 8 (store nav), 11 |
| §9 设置窗口（快捷键/外观/行为/历史三模式/自启动） | Task 13 |
| §10 设置 KV keys / get_settings / set_setting / clear_history / set_autostart | Task 2 |
| §10.3 历史清理 | Task 1 (enforce_history_limit), 5 (触发) |
| §10.4 来源捕获 | Task 4 |
| §10.5 自启动插件 | Task 2 |
| §10.6 窗口尺寸/主题应用 | Task 12 (theme), 14 (size) |
| §11 前端文件结构 | Tasks 6–13 |
| §13 验收 | Task 14 Step 4 |

