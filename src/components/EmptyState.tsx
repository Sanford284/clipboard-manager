import { observer } from 'mobx-react-lite';
import { clipboardStore } from '../stores/ClipboardStore';

export const EmptyState = observer(() => {
  const searching = clipboardStore.searchQuery.length > 0;
  return (
    <div className="flex flex-col items-center justify-center h-full text-muted gap-2 py-16">
      <svg width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.5}>
        <rect x="6" y="4" width="12" height="16" rx="2" />
        <path d="M9 8h6M9 12h6M9 16h3" strokeLinecap="round" />
      </svg>
      <p className="text-[13px]">
        {searching ? `没有匹配「${clipboardStore.searchQuery}」的记录` : '还没有剪切板记录，复制点什么吧'}
      </p>
    </div>
  );
});
