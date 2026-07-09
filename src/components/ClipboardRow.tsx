import { useEffect, useMemo } from 'react';
import { observer } from 'mobx-react-lite';
import type { ClipboardItem } from '../stores/ClipboardStore';
import { classify, formatTime } from '../lib/format';
import { Icon, PinIcon, TrashIcon } from './Icon';

interface Props {
  item: ClipboardItem;
  selected: boolean;
  showSource: boolean;
  onPaste: () => void;
  onPin: (e: React.MouseEvent) => void;
  onDelete: (e: React.MouseEvent) => void;
}

export const ClipboardRow = observer(({ item, selected, showSource, onPaste, onPin, onDelete }: Props) => {
  const kind = classify(item);

  const thumbUrl = useMemo(() => {
    if (kind !== 'image' || !item.thumb_content?.length) return null;
    const blob = new Blob([new Uint8Array(item.thumb_content)], { type: 'image/jpeg' });
    return URL.createObjectURL(blob);
  }, [kind, item.thumb_content]);

  useEffect(() => {
    return () => {
      if (thumbUrl) URL.revokeObjectURL(thumbUrl);
    };
  }, [thumbUrl]);

  const time = formatTime(item.created_at);
  const sourceTime = showSource && item.app_source ? `${item.app_source} · ${time}` : time;

  return (
    <div
      onClick={onPaste}
      data-id={item.id}
      className={`group flex items-center gap-2.5 h-[30px] px-3 cursor-pointer border-b border-border text-text text-[13px] ${
        selected ? 'bg-accent-soft' : 'hover:bg-surface-hover'
      }`}
    >
      {thumbUrl ? (
        <img src={thumbUrl} alt="" className="w-[18px] h-[18px] rounded-[5px] object-cover shrink-0" />
      ) : (
        <Icon kind={kind} />
      )}

      <span className="flex-1 min-w-0 truncate">{item.preview}</span>

      <span className="shrink-0 text-[11px] text-muted">{sourceTime}</span>

      <div className="flex items-center gap-1 shrink-0">
        <button
          onClick={onPin}
          title="置顶"
          className={`p-0.5 rounded transition-opacity ${
            item.pinned
              ? 'text-yellow-500 opacity-100'
              : 'text-muted hover:text-text opacity-0 group-hover:opacity-100'
          }`}
        >
          <PinIcon active={item.pinned} />
        </button>
        <button
          onClick={onDelete}
          title="删除"
          className="p-0.5 rounded text-muted hover:text-red-500 opacity-0 group-hover:opacity-100 transition-opacity"
        >
          <TrashIcon />
        </button>
      </div>
    </div>
  );
});
