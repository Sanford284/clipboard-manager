import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { applyTheme } from './lib/theme';

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

  useEffect(() => {
    applyTheme((settings.theme ?? 'light') as 'light' | 'dark');
  }, [settings.theme]);

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
