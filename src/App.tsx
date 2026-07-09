import { useEffect, useRef } from 'react';
import { observer } from 'mobx-react-lite';
import { clipboardStore } from './stores/ClipboardStore';
import { applyTheme } from './lib/theme';
import { useKeyboardNav } from './hooks/useKeyboardNav';
import { SearchBar } from './components/SearchBar';
import { FilterChips } from './components/FilterChips';
import { ClipboardList } from './components/ClipboardList';

const App = observer(() => {
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    applyTheme(clipboardStore.settings.theme);
  }, [clipboardStore.settings.theme]);

  useKeyboardNav({
    focusSearch: () => searchRef.current?.focus(),
  });

  return (
    <div className="h-screen flex flex-col bg-bg">
      <SearchBar ref={searchRef} />
      <FilterChips />
      <ClipboardList />
    </div>
  );
});

export default App;
