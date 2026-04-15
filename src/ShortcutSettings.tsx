import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import './styles.css';

const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta']);

function formatShortcutForDisplay(shortcut: string): string {
  return shortcut
    .replace(/CommandOrControl/g, '⌘')
    .replace(/Control/g, '⌃')
    .replace(/Shift/g, '⇧')
    .replace(/Alt/g, '⌥')
    .replace(/\+/g, '');
}

function ShortcutSettings() {
  const [currentShortcut, setCurrentShortcut] = useState('');
  const [newShortcut, setNewShortcut] = useState('');
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    invoke<string>('get_shortcut').then(setCurrentShortcut);
  }, []);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (!recording) return;
    e.preventDefault();

    const modifiers: string[] = [];
    if (e.metaKey) modifiers.push('CommandOrControl');
    else if (e.ctrlKey) modifiers.push('Control');
    if (e.shiftKey) modifiers.push('Shift');
    if (e.altKey) modifiers.push('Alt');

    // Ignore if only modifier keys are pressed
    if (MODIFIER_KEYS.has(e.key)) return;

    if (modifiers.length === 0) {
      setError('快捷键必须包含修饰键（⌘/⌃/⇧/⌥）');
      return;
    }

    const key = e.key.length === 1 ? e.key.toUpperCase() : e.key;
    const shortcutStr = [...modifiers, key].join('+');
    setNewShortcut(shortcutStr);
    setRecording(false);
    setError('');
  }, [recording]);

  const handleSave = async () => {
    const shortcutToSave = newShortcut || currentShortcut;
    try {
      await invoke('set_shortcut', { shortcut: shortcutToSave });
      setCurrentShortcut(shortcutToSave);
      setNewShortcut('');
      await getCurrentWindow().close();
    } catch (e) {
      setError(`无法注册快捷键: ${e}`);
    }
  };

  const handleCancel = async () => {
    await getCurrentWindow().close();
  };

  const handleResetDefault = () => {
    const defaultShortcut = 'CommandOrControl+Shift+V';
    setNewShortcut(defaultShortcut);
    setError('');
  };

  return (
    <div className="h-screen bg-gray-100 flex items-center justify-center p-4">
      <div className="bg-white rounded-lg shadow-lg p-6 w-full max-w-sm">
        <h2 className="text-lg font-semibold mb-4 text-gray-800">修改快捷键</h2>

        <div className="mb-4">
          <label className="block text-sm text-gray-600 mb-1">当前快捷键</label>
          <div className="text-lg font-mono text-gray-800">
            {formatShortcutForDisplay(currentShortcut)}
          </div>
        </div>

        <div className="mb-4">
          <label className="block text-sm text-gray-600 mb-1">新快捷键</label>
          <div
            tabIndex={0}
            onKeyDown={handleKeyDown}
            onClick={() => { setRecording(true); setError(''); }}
            onBlur={() => setRecording(false)}
            className={`w-full px-4 py-3 border-2 rounded-lg text-center font-mono text-lg cursor-pointer select-none ${
              recording
                ? 'border-blue-500 bg-blue-50 text-blue-700'
                : 'border-gray-300 bg-gray-50 text-gray-700'
            }`}
          >
            {recording
              ? '按下新的快捷键组合...'
              : newShortcut
                ? formatShortcutForDisplay(newShortcut)
                : '点击此处录入快捷键'}
          </div>
        </div>

        {error && (
          <p className="text-red-500 text-sm mb-4">{error}</p>
        )}

        <div className="flex gap-2">
          <button
            onClick={handleSave}
            disabled={!newShortcut}
            className="flex-1 px-4 py-2 bg-blue-500 text-white rounded-lg hover:bg-blue-600 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            保存
          </button>
          <button
            onClick={handleResetDefault}
            className="px-4 py-2 bg-gray-200 text-gray-700 rounded-lg hover:bg-gray-300"
          >
            恢复默认
          </button>
          <button
            onClick={handleCancel}
            className="px-4 py-2 bg-gray-200 text-gray-700 rounded-lg hover:bg-gray-300"
          >
            取消
          </button>
        </div>
      </div>
    </div>
  );
}

export default ShortcutSettings;
