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
