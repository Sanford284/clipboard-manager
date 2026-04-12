import { observer } from 'mobx-react-lite';
import { clipboardStore } from './stores/ClipboardStore';
import { useState } from 'react';
import './styles.css';

const App = observer(() => {
  const [searchInput, setSearchInput] = useState('');

  const handleSearch = (e: React.ChangeEvent<HTMLInputElement>) => {
    setSearchInput(e.target.value);
    clipboardStore.setSearch(e.target.value);
  };

  const handleFilterChange = (type: any) => {
    clipboardStore.setFilter(type);
  };

  const handleItemClick = async (id: number) => {
    await clipboardStore.pasteItem(id);
  };

  const handleDelete = async (id: number, e: React.MouseEvent) => {
    e.stopPropagation();
    await clipboardStore.deleteItem(id);
  };

  const handleTogglePin = async (id: number, e: React.MouseEvent) => {
    e.stopPropagation();
    await clipboardStore.togglePin(id);
  };

  return (
    <div className="h-screen bg-gray-100 flex flex-col">
      <div className="p-4 bg-white shadow">
        <input
          type="text"
          placeholder="搜索剪切板..."
          value={searchInput}
          onChange={handleSearch}
          className="w-full px-4 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
      </div>

      <div className="flex gap-2 p-4 bg-white border-b">
        {['all', 'text', 'image', 'file_path'].map(type => (
          <button
            key={type}
            onClick={() => handleFilterChange(type)}
            className={`px-4 py-2 rounded ${
              clipboardStore.filterType === type
                ? 'bg-blue-500 text-white'
                : 'bg-gray-200 text-gray-700'
            }`}
          >
            {type === 'all' ? '全部' : type === 'text' ? '文本' : type === 'image' ? '图片' : '文件'}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        {clipboardStore.filteredItems.map(item => (
          <div
            key={item.id}
            onClick={() => handleItemClick(item.id)}
            className="bg-white p-4 mb-2 rounded-lg shadow hover:shadow-md cursor-pointer transition"
          >
            <div className="flex justify-between items-start">
              <div className="flex-1">
                <p className="text-sm text-gray-600 mb-1">
                  {new Date(item.created_at).toLocaleString()}
                </p>
                <p className="text-gray-800 break-words">{item.preview}</p>
              </div>
              <div className="flex gap-2 ml-4">
                <button
                  onClick={(e) => handleTogglePin(item.id, e)}
                  className={`px-2 py-1 rounded ${
                    item.pinned ? 'bg-yellow-400' : 'bg-gray-200'
                  }`}
                >
                  📌
                </button>
                <button
                  onClick={(e) => handleDelete(item.id, e)}
                  className="px-2 py-1 bg-red-500 text-white rounded hover:bg-red-600"
                >
                  🗑️
                </button>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
});

export default App;

