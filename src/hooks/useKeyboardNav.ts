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

      // 数字 1-9：快速粘贴第 N 条（无修饰键；搜索框聚焦时不拦截，允许输入数字）
      const ae = document.activeElement;
      const typing = ae instanceof HTMLInputElement || ae instanceof HTMLTextAreaElement;
      if (!mod && !typing && /^[1-9]$/.test(e.key)) {
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
