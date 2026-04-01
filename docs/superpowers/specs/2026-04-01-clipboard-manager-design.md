# 跨平台剪切板管理工具设计文档

## 1. 项目概述

### 1.1 项目目标
开发一个类似 Ditto 的跨平台剪切板历史管理工具，支持 macOS 和 Windows，提供剪切板历史记录、搜索、预览和跨设备同步功能。

### 1.2 核心特性
- 自动捕获并保存剪切板历史记录（默认 1000 条，可配置）
- 全局快捷键唤起（macOS: Cmd+Shift+V，Windows: Ctrl+Shift+V）
- 支持多种数据类型：纯文本、富文本、图片、文件路径
- 列表展示 + 搜索过滤 + 预览
- 双击粘贴、删除、固定记录
- 持久化存储
- 跨设备同步（P2P + WebDAV，后续支持自建服务器）

### 1.3 技术栈
- **前端**: React 18 + TypeScript + TailwindCSS
- **状态管理**: MobX 6
- **后端**: Rust (Tauri 2.x)
- **数据库**: SQLite (rusqlite)
- **剪切板**: arboard
- **快捷键**: tauri-plugin-global-shortcut

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────┐
│                  Tauri Shell                 │
├──────────────────┬──────────────────────────┤
│   Frontend       │       Rust Backend       │
│   (React+MobX)   │                          │
│                  │  ┌────────────────────┐  │
│  ┌────────────┐  │  │ Clipboard Monitor  │  │
│  │ ClipStore  │◄─┼──│ (Event-driven)     │  │
│  │ (MobX)     │  │  └────────────────────┘  │
│  └─────┬──────┘  │                          │
│        │         │  ┌────────────────────┐  │
│  ┌─────▼──────┐  │  │ Storage Service    │  │
│  │ UI Layer   │  ├──│ (SQLite/rusqlite)  │  │
│  │ - List     │  │  └────────────────────┘  │
│  │ - Search   │  │                          │
│  │ - Preview  │  │  ┌────────────────────┐  │
│  └────────────┘  │  │ Sync Service       │  │
│                  │  │ (P2P / WebDAV)     │  │
│                  │  └────────────────────┘  │
├──────────────────┴──────────────────────────┤
│  tauri-plugin-global-shortcut               │
│  (Cmd+Shift+V / Ctrl+Shift+V)              │
└─────────────────────────────────────────────┘
```

### 2.2 核心模块职责

| 模块 | 职责 |
|------|------|
| **Clipboard Monitor** | Rust 端使用平台特定的事件驱动 API 监听剪切板变化（Windows: `AddClipboardFormatListener`，macOS: `NSPasteboard.changeCount` + 低频轮询 1-2s），检测到新内容时写入 SQLite 并通过 Tauri event 通知前端 |
| **Storage Service** | 管理 SQLite 数据库的 CRUD 操作，处理历史记录上限、持久化 |
| **Sync Service** | 负责 P2P 局域网发现和 WebDAV 同步（后续阶段实现） |
| **ClipStore (MobX)** | 前端状态管理，维护当前显示的剪切板列表、搜索状态、筛选条件 |
| **UI Layer** | React 组件，负责列表渲染、搜索框、预览面板、操作按钮 |

### 2.3 数据流

1. 用户在任意应用中复制内容
2. Clipboard Monitor 通过系统事件检测到剪切板变化
3. Rust 端将内容写入 SQLite，发送 `clipboard-changed` event
4. 前端 ClipStore 收到事件，更新 MobX observable 列表
5. UI 自动重新渲染，新记录出现在列表顶部

---

## 3. 剪切板监听实现

### 3.1 跨平台统一接口

```rust
// src-tauri/src/clipboard/mod.rs
pub trait ClipboardMonitor: Send {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send>) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
}

pub enum ClipboardContent {
    Text(String),
    RichText { plain: String, html: String },
    Image(Vec<u8>),
    FilePath(String),
}
```

### 3.2 平台特定实现

**Windows 实现** (`clipboard/windows.rs`):
```rust
use windows::Win32::System::DataExchange::*;
use windows::Win32::UI::WindowsAndMessaging::*;

pub struct WindowsClipboardMonitor {
    hwnd: HWND,
    running: Arc<AtomicBool>,
}

impl ClipboardMonitor for WindowsClipboardMonitor {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send>) -> Result<(), String> {
        // 1. 创建隐藏窗口接收消息
        // 2. 调用 AddClipboardFormatListener(hwnd)
        // 3. 消息循环处理 WM_CLIPBOARDUPDATE
        // 4. 读取剪切板内容并调用 callback
    }
}
```

**macOS 实现** (`clipboard/macos.rs`):
```rust
use cocoa::appkit::NSPasteboard;
use cocoa::foundation::NSAutoreleasePool;

pub struct MacOSClipboardMonitor {
    last_change_count: i64,
    running: Arc<AtomicBool>,
}

impl ClipboardMonitor for MacOSClipboardMonitor {
    fn start(&mut self, callback: Box<dyn Fn(ClipboardContent) + Send>) -> Result<(), String> {
        // 1. 获取 [NSPasteboard generalPasteboard]
        // 2. 每 1-2 秒检查 changeCount
        // 3. 如果 changeCount 变化，读取内容并调用 callback
        // 4. 使用 tokio::time::interval 实现低频轮询
    }
}
```

### 3.3 依赖 Crates

- **Windows**: `windows = "0.52"` (Win32 API 绑定)
- **macOS**: `cocoa = "0.25"`, `objc = "0.2"`
- **通用**: `arboard = "3.3"` (用于读取剪切板内容)

---

## 4. 数据模型与存储

### 4.1 SQLite 数据表

```sql
-- 剪切板记录表
CREATE TABLE clipboard_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type TEXT NOT NULL,        -- 'text' | 'rich_text' | 'image' | 'file_path'
    text_content TEXT,                  -- 纯文本/富文本内容
    html_content TEXT,                  -- HTML 格式（富文本时保存）
    blob_content BLOB,                  -- 图片二进制数据
    file_path   TEXT,                   -- 文件路径
    preview     TEXT,                   -- 预览文本（截取前200字符）或图片缩略图路径
    app_source  TEXT,                   -- 来源应用名称
    pinned      INTEGER DEFAULT 0,      -- 是否固定 (0/1)
    created_at  INTEGER NOT NULL,       -- 时间戳 (Unix ms)
    hash        TEXT NOT NULL UNIQUE    -- 内容哈希，用于去重
);

CREATE INDEX idx_created_at ON clipboard_items(created_at DESC);
CREATE INDEX idx_pinned ON clipboard_items(pinned);
CREATE INDEX idx_content_type ON clipboard_items(content_type);

-- 用户设置表
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

### 4.2 存储策略

- **去重**：通过 `hash` 字段（SHA-256），相同内容不重复存储，而是更新 `created_at` 将其置顶
- **上限管理**：当记录数超过用户配置的上限时，删除最早的非固定记录
- **图片存储**：小图片（<1MB）存为 BLOB，大图片存到独立文件目录，数据库保存路径引用
- **预览生成**：文本截取前 200 字符；图片生成 100x100 缩略图

### 4.3 MobX Store 结构

```typescript
// src/stores/ClipboardStore.ts
class ClipboardStore {
  // Observable state
  items: ClipboardItem[] = [];
  searchQuery: string = '';
  filterType: ContentType | 'all' = 'all';
  selectedId: number | null = null;
  isVisible: boolean = false;

  // Computed
  get filteredItems(): ClipboardItem[] {
    return this.items
      .filter(item => {
        if (this.filterType !== 'all' && item.contentType !== this.filterType) return false;
        if (this.searchQuery && !item.preview.toLowerCase().includes(this.searchQuery.toLowerCase())) return false;
        return true;
      })
      .sort((a, b) => {
        if (a.pinned !== b.pinned) return b.pinned - a.pinned;
        return b.createdAt - a.createdAt;
      });
  }

  get pinnedItems(): ClipboardItem[] {
    return this.items.filter(item => item.pinned);
  }

  get selectedItem(): ClipboardItem | null {
    return this.items.find(item => item.id === this.selectedId) || null;
  }

  // Actions
  async loadItems(): Promise<void> { ... }
  async deleteItem(id: number): Promise<void> { ... }
  async togglePin(id: number): Promise<void> { ... }
  async pasteItem(id: number): Promise<void> { ... }
  setSearch(query: string): void { ... }
  setFilter(type: ContentType | 'all'): void { ... }
  toggleVisibility(): void { ... }
}
```

### 4.4 Tauri Command 接口

```rust
#[tauri::command]
fn get_clipboard_items(
    limit: u32,
    offset: u32,
    search: Option<String>,
    content_type: Option<String>
) -> Result<Vec<ClipboardItem>, String>

#[tauri::command]
fn delete_clipboard_item(id: i64) -> Result<(), String>

#[tauri::command]
fn toggle_pin(id: i64) -> Result<(), String>

#[tauri::command]
fn paste_item(id: i64) -> Result<(), String>  // 将内容写入剪切板并模拟粘贴

#[tauri::command]
fn get_settings() -> Result<HashMap<String, String>, String>

#[tauri::command]
fn update_setting(key: String, value: String) -> Result<(), String>
```

---

## 5. UI 设计

### 5.1 主窗口布局

```
┌─────────────────────────────────────────┐
│  🔍 [搜索框]              [⚙️ 设置]     │
├─────────────────────────────────────────┤
│  [全部] [文本] [图片] [文件]  ← 筛选标签 │
├─────────────────────────────────────────┤
│  📌 固定项                               │
│  ┌───────────────────────────────────┐  │
│  │ 📄 Hello World                    │  │
│  │    2024-04-01 10:30              │  │
│  └───────────────────────────────────┘  │
│                                          │
│  📋 历史记录                             │
│  ┌───────────────────────────────────┐  │
│  │ 📄 const foo = 'bar'              │  │
│  │    2024-04-01 10:25              │  │
│  ├───────────────────────────────────┤  │
│  │ 🖼️ [图片预览缩略图]                │  │
│  │    2024-04-01 10:20              │  │
│  ├───────────────────────────────────┤  │
│  │ 📁 /Users/foo/document.pdf        │  │
│  │    2024-04-01 10:15              │  │
│  └───────────────────────────────────┘  │
│                                          │
│  [显示 50 / 1000 条]                    │
└─────────────────────────────────────────┘
```

### 5.2 交互行为

| 操作 | 行为 |
|------|------|
| **Cmd/Ctrl+Shift+V** | 唤起/隐藏主窗口 |
| **双击列表项** | 将内容写入剪切板并粘贴到当前活动窗口，关闭主窗口 |
| **Enter** | 同双击 |
| **右键菜单** | 显示：复制、删除、固定/取消固定、查看详情 |
| **搜索框输入** | 实时过滤列表（防抖 300ms） |
| **Esc** | 关闭主窗口 |
| **↑/↓ 方向键** | 选择上/下一项 |
| **点击固定图标** | 切换固定状态 |

### 5.3 预览面板

点击列表项时，右侧显示预览面板：

- **文本**：显示完整内容，支持语法高亮（检测代码语言）
- **图片**：显示原图，支持缩放
- **文件路径**：显示文件信息（大小、修改时间）
- **富文本**：渲染 HTML 预览

---

## 6. 跨设备同步

### 6.1 P2P 同步（现阶段）

**实现方案**：
- 使用 mDNS/Bonjour 进行局域网设备发现
- 建立 TCP 连接进行数据传输
- 增量同步：只传输新增/修改的记录

**Rust Crates**：
- `mdns-sd = "0.10"` - mDNS 服务发现
- `tokio = { version = "1", features = ["net", "sync"] }` - 异步网络

**同步流程**：
1. 设备 A 启动时广播 mDNS 服务
2. 设备 B 发现设备 A，建立 TCP 连接
3. 交换最后同步时间戳
4. 传输增量数据（JSON 格式）
5. 双向合并，解决冲突（最新时间戳优先）

### 6.2 WebDAV 同步（现阶段）

**实现方案**：
- 用户配置 WebDAV 服务器地址、用户名、密码
- 定期上传本地数据库快照到 WebDAV
- 下载远程快照并合并

**Rust Crates**：
- `reqwest = { version = "0.11", features = ["blocking"] }` - HTTP 客户端

**同步流程**：
1. 每 5 分钟（可配置）检查远程是否有更新
2. 下载远程 SQLite 文件
3. 合并到本地数据库（按时间戳去重）
4. 上传本地最新快照

### 6.3 自建服务器同步（后续）

预留接口，后续实现：
- RESTful API 或 WebSocket 连接
- 服务端使用 Node.js/Go 实现
- 支持多设备实时同步

---

## 7. 项目结构

```
clipboard-manager/
├── src/                          # React 前端
│   ├── main.tsx                  # 入口文件
│   ├── App.tsx                   # 主应用组件
│   ├── stores/
│   │   └── ClipboardStore.ts     # MobX store
│   ├── components/
│   │   ├── ClipboardList.tsx     # 列表组件
│   │   ├── SearchBar.tsx         # 搜索框
│   │   ├── PreviewPanel.tsx      # 预览面板
│   │   └── SettingsDialog.tsx    # 设置对话框
│   ├── hooks/
│   │   └── useTauriEvent.ts      # Tauri 事件监听 hook
│   └── styles/
│       └── globals.css           # TailwindCSS 样式
│
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # Tauri 入口
│   │   ├── clipboard/
│   │   │   ├── mod.rs            # 统一接口定义
│   │   │   ├── windows.rs        # Windows 实现
│   │   │   └── macos.rs          # macOS 实现
│   │   ├── storage/
│   │   │   ├── mod.rs            # 数据库操作
│   │   │   └── models.rs         # 数据模型
│   │   ├── sync/
│   │   │   ├── mod.rs            # 同步接口
│   │   │   ├── p2p.rs            # P2P 同步
│   │   │   └── webdav.rs         # WebDAV 同步
│   │   └── commands.rs           # Tauri commands
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── package.json
├── tsconfig.json
├── tailwind.config.js
└── vite.config.ts
```

---

## 8. 开发阶段规划

### 阶段 1：核心功能（MVP）
- ✅ Tauri 项目初始化
- ✅ 剪切板监听（Windows + macOS）
- ✅ SQLite 存储
- ✅ 基础 UI（列表 + 搜索）
- ✅ 全局快捷键
- ✅ 双击粘贴

### 阶段 2：增强功能
- ✅ 预览面板
- ✅ 固定记录
- ✅ 类型筛选
- ✅ 图片支持
- ✅ 设置界面

### 阶段 3：同步功能
- ✅ P2P 局域网同步
- ✅ WebDAV 同步
- ⏳ 自建服务器同步（后续）

### 阶段 4：优化与打磨
- ⏳ 性能优化
- ⏳ 错误处理
- ⏳ 单元测试
- ⏳ 打包与分发

---

## 9. 技术风险与挑战

### 9.1 剪切板监听
- **macOS 限制**：没有真正的事件驱动 API，需要低频轮询
- **解决方案**：1-2 秒轮询间隔，平衡性能和实时性

### 9.2 图片存储
- **大图片性能**：BLOB 存储可能导致数据库膨胀
- **解决方案**：大于 1MB 的图片存为独立文件

### 9.3 跨平台粘贴
- **模拟粘贴**：需要模拟 Cmd/Ctrl+V 按键
- **解决方案**：使用 `enigo` crate 模拟键盘输入

### 9.4 同步冲突
- **多设备同步**：可能出现数据冲突
- **解决方案**：最新时间戳优先，固定记录优先级更高

---

## 10. 非功能需求

### 10.1 性能
- 启动时间 < 2 秒
- 快捷键响应 < 100ms
- 搜索响应 < 50ms（1000 条记录）

### 10.2 资源占用
- 内存占用 < 100MB（空闲状态）
- 磁盘占用 < 50MB（不含图片）

### 10.3 兼容性
- macOS 11.0+
- Windows 10+

---

## 11. 后续扩展

- 🔮 加密存储（AES-256）
- 🔮 敏感内容过滤
- 🔮 分类管理（标签系统）
- 🔮 快捷短语（常用文本快速插入）
- 🔮 OCR 图片文字识别
- 🔮 云存储同步（S3、Google Drive）
- 🔮 浏览器扩展集成
