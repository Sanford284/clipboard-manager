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
