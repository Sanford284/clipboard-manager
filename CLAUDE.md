# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

A desktop clipboard manager built with **Tauri 2** (Rust backend) + **React 19** + **TypeScript** + **Vite**. Targets macOS and Windows only — there is no Linux clipboard monitor implementation. UI is Chinese-localized. Styling is Tailwind CSS v4 (via `@tailwindcss/postcss`, imported in `src/styles.css`).

## Commands

```bash
npm run tauri dev      # Full app dev (runs vite on :1420 + launches Tauri shell). Use this for normal work.
npm run dev            # Frontend-only Vite dev server (port 1420, strictPort)
npm run build          # tsc typecheck + vite build (production frontend bundle)
npm run tauri build    # Production bundle (installer/.app/.msi)
cargo test --manifest-path src-tauri/Cargo.toml   # Rust tests (none currently defined)
```

There is **no linter and no test suite configured**. The frontend `tsc` run during `npm run build` is the only typecheck gate. After editing Rust, rebuild happens automatically inside `tauri dev`; after editing frontend, Vite HMR handles it.

Tauri requires Vite on fixed port **1420** (`strictPort: true`) — if it's taken, the dev server errors rather than bumping ports.

## Architecture

### Two windows, one frontend bundle
Tauri serves a single `index.html`. `src/main.tsx` reads a `?window=` query param to decide which root component to render:
- no param → `App.tsx` (main clipboard list, webview label `"main"`)
- `?window=settings` → `ShortcutSettings.tsx` (shortcut editor, webview label `"settings"`, created on demand from the tray menu in `lib.rs`)

When adding a third window, follow this pattern: a new query-param branch in `main.tsx` plus a `WebviewWindowBuilder` in Rust that loads `index.html?window=<name>`.

### Backend layout (`src-tauri/src/`)
- `lib.rs` — `run()` wires everything: opens the DB, spawns the clipboard monitor thread, registers the global shortcut, builds the system-tray menu, installs window-event handlers, and registers the `invoke_handler` commands. **This is the integration point** — any new Tauri command must be added both in `commands.rs` and to the `generate_handler!` list here.
- `commands.rs` — the `#[tauri::command]` functions callable from the frontend via `invoke(...)`.
- `clipboard/` — platform-specific polling monitors (`macos.rs`, `windows.rs`) behind a `ClipboardMonitor` trait. Selected via `create_monitor()` with `#[cfg(target_os = ...)]`.
- `storage/` — SQLite via `rusqlite` (bundled). `models.rs` has `ClipboardItem`; `mod.rs` has the `Database` wrapper.

Shared backend state is passed to the frontend through `tauri::State`:
- `Arc<Mutex<Database>>` — all DB access
- `PreviousApp` = `Arc<Mutex<Option<String>>>` — bundle id of the app that was focused before the user summoned the window (used to restore focus on paste)
- `MonitorPaused` = `Arc<AtomicBool>` — pause flag for the monitor thread

### Frontend state
`src/stores/ClipboardStore.ts` is a single MobX store (`makeAutoObservable`), instantiated as a singleton. It calls Tauri commands and listens for the `clipboard-changed` event emitted from Rust. Components are wrapped with `observer()` from `mobx-react-lite`.

### Data flow: clipboard capture
1. Monitor thread polls the OS clipboard every ~1s (Windows) / 1.5s (macOS) via `arboard`.
2. On change → constructs a `ClipboardItem`, calls `Database::insert_item`.
3. `insert_item` dedupes on the SHA256 `hash` column (UNIQUE) — a repeat copy bumps `created_at` to float it back to the top instead of inserting.
4. Emits `clipboard-changed` event → MobX store reloads.

### Data flow: paste (`paste_item` command)
Critical sequence — order matters: write text to clipboard via `arboard` → record/hide main window → spawn a thread that re-activates the previously-frontmost app (macOS: `NSRunningApplication` by bundle id) and simulates Cmd/Ctrl+V (macOS: `osascript` AppleScript; Windows: `enigo`). Don't reorder these or the paste lands in the wrong window.

## Non-obvious facts

- **Database path uses the OS app-data dir** (`app.path().app_data_dir()`), resolved inside `run()`'s `setup` closure because it needs the app handle. macOS: `~/Library/Application Support/com.clipboard.manager/clipboard.db`; Windows: `%APPDATA%\com.clipboard.manager\clipboard.db`. An earlier version used a CWD-relative `data/` path, which crashed the bundled `.app` (LaunchServices launches it with CWD `/`). Do not revert to `current_dir()` for the DB.
- **Main window starts hidden** (`visible: false` in `tauri.conf.json`) and is shown only via the global shortcut or tray menu. It **never truly closes**: `CloseRequested` and `Focused(false)` (blur) both call `window.hide()` and `api.prevent_close()`. The settings window behaves like a normal window and does close.
- **Global shortcut is user-configurable and persisted.** Stored in the `settings` table under key `"shortcut"`. Default is `CommandOrControl+Shift+V` (macOS) / `Control+Shift+V` (others). Changing it (`set_shortcut` command) unregisters the old and registers the new with an identical handler (the show/hide toggle). Both `lib.rs` and `commands.rs` contain near-duplicate handler closures — keep them in sync when modifying shortcut behavior.
- **Monitoring is text-only in practice.** The `ClipboardContent` enum has `RichText`/`Image`/`FilePath` variants, but the monitors only emit `ClipboardContent::Text`, and only `Text` and `Image` are handled in `lib.rs`'s capture closure. Image handling is wired but the monitor never produces images.
- **`get_items` builds its SQL query via string interpolation** for `content_type` and `search` (only `limit`/`offset` are parameterized). These values come from the frontend, so treat any extension here as untrusted input.
- **Cross-platform deps**: macOS uses `cocoa`/`objc` for app activation and bundle-id lookup; Windows uses `enigo` for keystroke simulation and the `windows` crate. Both gated by `#[cfg(target_os = ...)]`.
