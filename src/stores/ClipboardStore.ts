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
