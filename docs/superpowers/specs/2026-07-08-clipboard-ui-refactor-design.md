# 剪切板管理器 UI 重构 · 设计文档

- 日期：2026-07-08
- 范围：前端 UI 重构 + 必要的后端支撑（设置、来源捕获、图片抓取、历史清理、自启动）
- 目标受众：实现者（本文件 → writing-plans → 实现）

## 1. 目标

1. **提高信息密度**：每条记录占用空间大幅缩小，视口内可见 ~15–18 条。
2. **替换难看的图标**：用统一的线性图标系统取代 emoji（📌 🗑️ 等）。
3. **预览占满整行宽度**：省略号只在右边缘出现，不再半截截断。
4. **新增配置**：默认窗口宽高、深色模式、来源应用显示、历史保留策略、开机自启动。
5. **新增能力**：图片抓取与缩略图显示、完整键盘流。

## 2. 非目标（YAGNI，本期不做）

- 列表虚拟化（1000 条以内直接渲染；若实测卡顿再补）。
- 「记住上次窗口尺寸」（只做默认宽高配置）。
- 富文本（HTML）渲染展示（保留字段，不渲染）。
- 代码类型的过滤芯片（识别太模糊；仅显示代码图标，不过滤）。
- 跨设备同步、收藏分组、OCR 等额外功能。
- 图片缩略图以外的图片编辑/预览大图。

## 3. 已锁定决策

| 维度 | 决策 |
|---|---|
| 形态 | A · 紧凑单行列表，每行 ~30px |
| 图标风格 | V3 · 线性图标 + 按类型上色的 18px 圆角方块 |
| 预览宽度 | 占满整行（flex:1），末尾省略 |
| 窗口 | 保持可缩放；新增「默认宽度/高度」配置 |
| 深色模式 | 手动切换（浅色/深色，不跟随系统） |
| 键盘流 | 完整：↑↓ 选择、Enter 粘贴、1–9 快速粘贴、Esc、⌘/Ctrl+F |
| 图片 | 抓取 + 缩略图（原图存库用于粘贴） |
| 重构力度 | 全面组件化 |

## 4. 主窗口设计

### 4.1 布局

```
┌──────────────────────────────────────────────┐
│ 🔍 搜索剪切板…                      ⌘F       │  SearchBar
├──────────────────────────────────────────────┤
│ [全部] 文本 链接 图片 文件                     │  FilterChips
├──────────────────────────────────────────────┤
│ ▤  下午三点会议改到会议室A，请大家准时…  刚刚 │
│ ⛓  github.com/tauri-apps/tauri/blo…     2分前 │  ClipboardRow × N
│ ❮  const fn poll() -> bool { self.la…   5分前 │  (~30px / 行)
│ ▤  你好，附件是本周周报…            微信 14:30 │
│  ...                                          │
└──────────────────────────────────────────────┘
```

### 4.2 行结构（ClipboardRow）

单行 flex 布局，从左到右：

```
[18px 类型图标/缩略图] [预览 flex:1 单行省略] [来源·时间 右贴] [hover: 置顶 删除]
```

- 行高 ~30–32px，`px-3`，底边 1px hairline 分割。
- **点击整行 = 粘贴**（保留现有 `paste_item` 行为）。
- **hover**：行背景变浅；右侧露出「置顶」「删除」两个图标按钮。
- **置顶项**：常驻金色图钉标记（不 hover 也可见），置顶区排在最前（store 现有排序逻辑保留：pinned 优先，再按 created_at 降序）。
- **选中项**（键盘导航）：与 hover 区分的选中高亮（如左侧 2px 强调条 + 浅底）。

### 4.3 时间格式（相对智能）

`format.ts` 实现：

- < 60s → `刚刚`
- < 60min → `N分钟前`
- 当天 → `HH:mm`
- 昨天 → `昨天 HH:mm`
- 更早 → `M月D日`

### 4.4 来源应用显示（可配置 `show_source`）

- 开启时，时间位显示 `微信 · 刚刚`；关闭时只显示 `刚刚`。始终单行，不额外挤占预览。
- `app_source` 为空时退化为只显示时间。

### 4.5 过滤芯片（FilterChips）

类型：`全部 / 文本 / 链接 / 图片 / 文件`

- 全部 = 不限
- 文本 = `content_type = text`
- 链接 = `content_type = text` 且内容匹配 URL 正则（前端/后端均可判定，见 §10）
- 图片 = `content_type = image`
- 文件 = `content_type = file_path`
- 选中项为填充强调色 pill，其余为浅灰。

### 4.6 空状态（EmptyState）

- 无任何记录：图标 + `还没有剪切板记录，复制点什么吧`
- 搜索无结果：`没有匹配「{query}」的记录`

## 5. 图标系统（Icon.tsx）

统一线性 SVG 图标 + 按类型上色的 18px 圆角方块（`flex:0 0 auto`）：

| 类型 | 浅色（底/字） | 深色（底/字） | 触发 |
|---|---|---|---|
| 文本 | `#eef2ff / #4f46e5` | `#312e81 / #a5b4fc` | 默认文本 |
| 链接 | `#ecfdf5 / #059669` | `#064e3b / #6ee7b7` | 文本匹配 URL |
| 代码 | `#fff7ed / #ea580c` | `#7c2d12 / #fdba74` | 文本含代码特征（仅图标） |
| 文件 | `#eff6ff / #2563eb` | `#1e3a8a / #93c5fd` | `content_type=file_path` |
| 图片 | `#fdf2f8 / #db2777` | `#831843 / #f9a8d4` | `content_type=image` |

- 类型判定集中在 `lib/format.ts` 的 `classify(item)`：image/file 按 content_type；text 内部再分 plain/link/code。
- URL 正则：`/^https?:\/\/\S+/i`（取整行为 URL 即判定为链接）。
- 代码特征（轻量启发式，仅用于图标，不用于过滤）：包含 `function|=>|;\s*$|^\s*{|^\s*}` 或常见关键字组合。
- 图片行用**缩略图**替代图标方块（见 §6）。
- 操作图标（置顶/删除/搜索）也走同一组件库，单色、无底色。

## 6. 图片支持

### 6.1 捕获（监控层）

- `macos.rs` / `windows.rs`：每次轮询，先尝试读图片；若剪贴板当前为图片且与上次不同，发 `ClipboardContent::Image(Vec<u8> PNG bytes)`；否则按现状读文本。
- 单张大小上限 `MAX_IMAGE_BYTES = 2 * 1024 * 1024`（2MB）。超限则跳过本次图片捕获（不写库，记日志），避免库膨胀。

### 6.2 存储（lib.rs 捕获闭包的 Image 分支）

现 lib.rs:116 已有 Image 分支（存 `blob_content`、预览 `[Image]`），改造为：

1. 解码图片获取 `{width, height}`（用 `image` crate）。
2. 生成缩略图（最大边 256px，JPEG quality 80）→ `thumb_content`。
3. 原图字节 → `blob_content`。
4. `preview = "图片 {w}×{h}"`。
5. `content_type = "image"`，`app_source` 同文本路径填充。

### 6.3 列表查询与显示

- `get_clipboard_items`（列表查询）**SELECT 增加 `thumb_content`、移除 `blob_content`**——避免把大量原图字节传到前端。
- 前端把 `thumb_content`（`number[]`）转 Blob → object URL 渲染为 ~28px 缩略图，占行左侧图标位。
- 行预览文本显示 `图片 1920×1080`。

### 6.4 粘贴

- 现有 `paste_item(id)` 需要 item 的原图字节；由于列表查询已不含 `blob_content`，新增 `Database::get_item_by_id(id)`（SELECT **含 `blob_content`**）专供 `paste_item` 使用。
- 图片分支读 `blob_content` 经 `arboard` 写回剪贴板；文本分支不变。前端无需持有原图。

### 6.5 Schema 迁移

- 新增列 `thumb_content BLOB NULL`。
- `Database` 打开时执行轻量迁移：`PRAGMA table_info(clipboard_items)` 检查，缺失则 `ALTER TABLE clipboard_items ADD COLUMN thumb_content BLOB`。
- `get_items` SELECT 列表与 `INSERT` 参数列表相应增加 `thumb_content`。

## 7. 主题（手动浅色/深色）

- Tailwind v4 class 模式：`styles.css` 增加 `@custom-variant dark (&:where(.dark, .dark *))`。
- 设置存 `theme = "light" | "dark"`；`lib/theme.ts` 在启动与切换时给根节点加/去 `.dark`。
- 所有颜色走语义变量（在 `styles.css` 用 `@theme` / CSS 变量定义 `--bg`、`--surface`、`--text`、`--muted`、`--border`、`--accent`，深浅两套），组件只引用变量。
- 切换即时生效，无需重启。

## 8. 键盘导航（useKeyboardNav）

主窗口挂全局 keydown：

| 键 | 行为 |
|---|---|
| `↑` / `↓` | 在可见（过滤后）项间移动选中；自动 scrollIntoView；搜索框聚焦时同样生效 |
| `Enter` | 粘贴当前选中项（无选中时粘贴首条） |
| `1`–`9` | 直接粘贴第 N 条可见项 |
| `Esc` | 搜索非空 → 清空搜索；否则隐藏窗口（现有 hide 行为） |
| `⌘/Ctrl + F` | 聚焦搜索框 |

- store 增加 `selectedId`（已有字段）+ `moveSelection(±1)` / `clamp`；切换过滤或搜索后选中重置到首条。
- 搜索框聚焦时除上述全局键外，普通字符照常输入。
- 选中态视觉见 §4.2。

## 9. 设置窗口（Settings.tsx，由 ShortcutSettings 升级）

- 查询参数仍是 `?window=settings`（main.tsx 路由不变）。
- 分区标题堆叠（非 tab，结构简单）：

**快捷键**（现有逻辑保留）：当前快捷键 + 录入 + 恢复默认。

**外观**：
- 主题：浅色 / 深色（单选，即时生效）
- 默认窗口宽度：数字输入 px，范围 320–1200，默认 800
- 默认窗口高度：数字输入 px，范围 400–1400，默认 600

**行为**：
- 每行显示来源应用：开关
- 历史保留（segmented）：`自动保留最近 N 条`（N 数字输入，范围 50–10000，默认 500）/ `永不清除` / `手动清除`（配「立即清空」按钮，清空所有非置顶项）
- 开机自启动：开关

- 各项即时写设置表；**默认宽高在下次启动应用时生效**——主窗口在应用启动时创建一次，会话内 show/hide 复用同一窗口对象、不重建，故用户当次的拖拽缩放在本次运行内持续，下次启动才按新默认值创建。
- 底部保留「保存/取消」用于快捷键录入场景；其余项改动即存。

## 10. 后端改动

**新增 Rust 依赖**：`image`（图片解码 + 缩略图，见 §6.2）、`tauri-plugin-autostart`（开机自启，见 §10.5）。均需加到 `src-tauri/Cargo.toml`，后者还需在 `lib.rs` 注册插件与 capabilities。

### 10.1 设置存储（复用现有 KV 表）

新增 key（均为字符串）：

| key | 取值 |
|---|---|
| `theme` | `light` / `dark` |
| `window_width` | px 整数字符串 |
| `window_height` | px 整数字符串 |
| `show_source` | `true` / `false` |
| `history_mode` | `auto` / `never` / `manual` |
| `history_limit` | N（仅 mode=auto 用） |
| `autostart` | `true` / `false` |

### 10.2 新增 Tauri 命令（均需加入 lib.rs `generate_handler!`）

- `get_settings() -> HashMap<String,String>`：一次返回全部 key（缺省值由前端/后端约定补齐）
- `set_setting(key: String, value: String)`：通用写
- `clear_history()`：删除所有 `pinned = 0` 的记录（手动清除）
- `set_autostart(enabled: bool)`：通过 `tauri-plugin-autostart` 实现

### 10.3 历史清理

- `history_mode = auto` 且 `history_limit = N` 时，在 `Database::insert_item` 成功后执行：删除 `pinned = 0` 且不在「最近 N 条（按 created_at 降序）」内的记录。
- `never` / `manual` 不自动清理；`manual` 由 `clear_history` 命令触发。
- 清理始终保留置顶项。

### 10.4 来源应用捕获

- 监控线程检测到变化构造 `ClipboardItem` 时，取前台 app 名写入 `app_source`：
  - macOS：`NSWorkspace.sharedWorkspace.frontmostApplication.localizedName`
  - Windows：前台窗口进程名
- 取不到时回落 `None`（行上不显示来源，不报错）。
- 现 lib.rs:107/110（文本）、lib.rs:122/125（图片）的 `app_source: None` 改为实际值。

### 10.5 自启动

- `Cargo.toml` 加 `tauri-plugin-autostart`；`lib.rs` 注册插件；`set_autostart` 命令封装其 `enable/disable`。
- 设置窗开关联动该命令。

### 10.6 主窗口尺寸与主题应用

- 启动时 `get_settings` 读取；**应用启动创建主窗口时**用 `window_width/height` 设定初始尺寸（夹到合法范围）；会话内窗口对象复用，用户缩放持续到退出。`theme` 决定 `.dark`。
- 现有窗口事件（hide on blur/close）不变。

## 11. 前端文件结构

```
src/
  main.tsx                  # 路由不变（?window=settings）
  App.tsx                   # 主窗口壳：组合组件 + 键盘 handler + 主题应用
  components/
    SearchBar.tsx
    FilterChips.tsx
    ClipboardList.tsx
    ClipboardRow.tsx
    EmptyState.tsx
    Icon.tsx                # 类型→SVG+配色；操作图标；dark 变体
  hooks/
    useKeyboardNav.ts
  lib/
    theme.ts                # 读取/应用 .dark
    format.ts               # 时间相对化、classify(类型)、URL 判定
  stores/ClipboardStore.ts  # 加 moveSelection/clamp、selectedId 已有
  Settings.tsx              # 由 ShortcutSettings.tsx 升级
  styles.css                # @custom-variant dark + 语义色变量
```

- `ShortcutSettings.tsx` 重命名为 `Settings.tsx`（main.tsx 路由 import 同步）。
- `App.css`（Vite 模板残留）可清理或保留；不阻塞。

## 12. 边界与数据流

- **窗口尺寸**：拖拽缩放照旧；默认宽高仅用于「未手动调整过」的首次/重置打开。
- **图片捕获失败/超大**：跳过，不写库；不影响文本捕获。
- **缩略图 object URL**：组件卸载或列表刷新时 `URL.revokeObjectURL`，避免内存泄漏。
- **搜索**：前端 store 过滤（现状）+ 后端 `search` 参数（现状）并存。
- **深色模式**：所有自绘色（图标色块、选中高亮、hover、hairline）走语义变量。
- **配置校验**：宽高超范围自动夹上下限；`history_limit` 非法回落默认 500。
- **迁移幂等**：`thumb_content` 列添加用 table_info 守卫，重复执行安全。

## 13. 验收要点（供后续 plan/UAT）

- 视口内可见 ~15–18 条；预览占满整行、仅末尾省略。
- 图标为线性上色方块，无 emoji。
- 深色/浅色手动切换即时生效，两套配色均可用。
- 键盘：↑↓/Enter/1–9/Esc/⌘F 全部可用。
- 复制图片 → 行内出现缩略图；点击图片行 → 原图粘贴到目标 app。
- 设置项全部生效：默认宽高（下次唤起）、来源显示、历史保留三模式（auto 自动清理、manual 清空按钮、never 不动）、开机自启动。
- 行显示来源应用（开启时）。
