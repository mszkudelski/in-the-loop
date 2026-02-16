import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Item } from '../types';
import { ItemCard } from './ItemCard';
import { AddItemForm } from './AddItemForm';
import { Settings } from './Settings';

export function Dashboard() {
  const [items, setItems] = useState<Item[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [filter, setFilter] = useState<Item['type'] | 'all'>('all');

  useEffect(() => {
    loadItems();

    // Listen for item updates from backend
    const unlisten = listen('item-updated', (event) => {
      console.log('Item updated:', event);
      loadItems();
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const loadItems = async () => {
    try {
      const loadedItems: Item[] = await invoke('get_items', { archived: false });
      setItems(loadedItems);
    } catch (error) {
      console.error('Failed to load items:', error);
    }
  };

  const handleRemove = (id: string) => {
    setItems(items.filter(item => item.id !== id));
  };

  const filteredItems = filter === 'all' 
    ? items 
    : items.filter(item => item.type === filter);

  const typeCounts = {
    all: items.length,
    slack_thread: items.filter(i => i.type === 'slack_thread').length,
    github_action: items.filter(i => i.type === 'github_action').length,
    github_pr: items.filter(i => i.type === 'github_pr').length,
    copilot_agent: items.filter(i => i.type === 'copilot_agent').length,
    cli_session: items.filter(i => i.type === 'cli_session').length,
  };

  return (
    <div className="container">
      <h1>In The Loop</h1>
      <p style={{ color: '#999', marginBottom: '24px' }}>
        Track your async work items in one place
      </p>

      <div style={{ marginBottom: '24px', display: 'flex', justifyContent: 'space-between' }}>
        <button onClick={() => setShowSettings(!showSettings)}>
          {showSettings ? 'Hide Settings' : 'Show Settings'}
        </button>
        <button onClick={loadItems}>Refresh</button>
      </div>

      {showSettings && <Settings />}

      <AddItemForm onItemAdded={loadItems} />

      <div style={{ marginBottom: '16px', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
        <button 
          onClick={() => setFilter('all')}
          style={{ 
            borderColor: filter === 'all' ? '#646cff' : 'transparent',
            opacity: filter === 'all' ? 1 : 0.7
          }}
        >
          All ({typeCounts.all})
        </button>
        <button 
          onClick={() => setFilter('slack_thread')}
          style={{ 
            borderColor: filter === 'slack_thread' ? '#646cff' : 'transparent',
            opacity: filter === 'slack_thread' ? 1 : 0.7
          }}
        >
          💬 Slack ({typeCounts.slack_thread})
        </button>
        <button 
          onClick={() => setFilter('github_action')}
          style={{ 
            borderColor: filter === 'github_action' ? '#646cff' : 'transparent',
            opacity: filter === 'github_action' ? 1 : 0.7
          }}
        >
          ⚙️ Actions ({typeCounts.github_action})
        </button>
        <button 
          onClick={() => setFilter('github_pr')}
          style={{ 
            borderColor: filter === 'github_pr' ? '#646cff' : 'transparent',
            opacity: filter === 'github_pr' ? 1 : 0.7
          }}
        >
          🔀 PRs ({typeCounts.github_pr})
        </button>
        <button 
          onClick={() => setFilter('copilot_agent')}
          style={{ 
            borderColor: filter === 'copilot_agent' ? '#646cff' : 'transparent',
            opacity: filter === 'copilot_agent' ? 1 : 0.7
          }}
        >
          🤖 Copilot ({typeCounts.copilot_agent})
        </button>
        <button 
          onClick={() => setFilter('cli_session')}
          style={{ 
            borderColor: filter === 'cli_session' ? '#646cff' : 'transparent',
            opacity: filter === 'cli_session' ? 1 : 0.7
          }}
        >
          💻 CLI ({typeCounts.cli_session})
        </button>
      </div>

      {filteredItems.length === 0 ? (
        <div style={{ 
          padding: '40px', 
          textAlign: 'center', 
          color: '#999',
          border: '2px dashed #333',
          borderRadius: '8px'
        }}>
          {items.length === 0 
            ? 'No items tracked yet. Add a URL above to get started!' 
            : 'No items in this category'}
        </div>
      ) : (
        filteredItems.map(item => (
          <ItemCard key={item.id} item={item} onRemove={handleRemove} />
        ))
      )}
    </div>
  );
}
