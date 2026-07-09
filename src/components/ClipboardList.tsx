import { useEffect, useRef } from 'react';
import { observer } from 'mobx-react-lite';
import { clipboardStore } from '../stores/ClipboardStore';
import { ClipboardRow } from './ClipboardRow';
import { EmptyState } from './EmptyState';

export const ClipboardList = observer(() => {
  const items = clipboardStore.filteredItems;
  const containerRef = useRef<HTMLDivElement>(null);

  // 选中项滚动入视口
  useEffect(() => {
    if (clipboardStore.selectedId == null) return;
    const el = containerRef.current?.querySelector(`[data-id="${clipboardStore.selectedId}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [clipboardStore.selectedId]);

  if (items.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto bg-bg">
        <EmptyState />
      </div>
    );
  }

  return (
    <div ref={containerRef} className="flex-1 overflow-y-auto bg-surface">
      {items.map((item) => (
        <ClipboardRow
          key={item.id}
          item={item}
          selected={item.id === clipboardStore.selectedId}
          showSource={clipboardStore.settings.show_source}
          onPaste={() => clipboardStore.pasteItem(item.id)}
          onPin={(e) => {
            e.stopPropagation();
            clipboardStore.togglePin(item.id);
          }}
          onDelete={(e) => {
            e.stopPropagation();
            clipboardStore.deleteItem(item.id);
          }}
        />
      ))}
    </div>
  );
});
