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
