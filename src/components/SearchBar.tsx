import { forwardRef } from 'react';
import { observer } from 'mobx-react-lite';
import { clipboardStore } from '../stores/ClipboardStore';
import { SearchIcon } from './Icon';

interface Props {
  onCmdF?: () => void;
}

export const SearchBar = observer(forwardRef<HTMLInputElement, Props>((_props, ref) => {
  return (
    <div className="flex items-center gap-2 px-3 h-11 border-b border-border bg-surface">
      <SearchIcon className="text-muted shrink-0" />
      <input
        ref={ref}
        type="text"
        placeholder="搜索剪切板…"
        value={clipboardStore.searchQuery}
        onChange={(e) => clipboardStore.setSearch(e.target.value)}
        className="flex-1 min-w-0 bg-transparent outline-none text-text placeholder:text-muted text-[13px]"
      />
      <kbd className="text-[11px] text-muted shrink-0 border border-border rounded px-1.5 py-0.5">
        ⌘F
      </kbd>
    </div>
  );
}));
