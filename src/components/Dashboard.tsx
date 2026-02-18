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

  const sortedItems = [...filteredItems].sort((a, b) => {
    if (a.status === 'archived' && b.status !== 'archived') return 1;
    if (a.status !== 'archived' && b.status === 'archived') return -1;
    return 0;
  });

  const typeCounts = {
    all: items.length,
    slack_thread: items.filter(i => i.type === 'slack_thread').length,
    github_action: items.filter(i => i.type === 'github_action').length,
    github_pr: items.filter(i => i.type === 'github_pr').length,
    copilot_agent: items.filter(i => i.type === 'copilot_agent').length,
    cli_session: items.filter(i => i.type === 'cli_session').length,
    opencode_session: items.filter(i => i.type === 'opencode_session').length,
  };

  return (
    <div className="container">
      <h1 className="page-title">In The Loop</h1>
      <p className="page-subtitle">
        Track your async work items in one place
      </p>

      <div className="toolbar">
        <button onClick={() => setShowSettings(!showSettings)}>
          {showSettings ? 'Hide Settings' : 'Show Settings'}
        </button>
        <button onClick={loadItems}>Refresh</button>
      </div>

      {showSettings && <Settings />}

      <AddItemForm onItemAdded={loadItems} />

      <div className="filter-row">
        <button 
          onClick={() => setFilter('all')}
          className={`filter-chip ${filter === 'all' ? 'active' : ''}`}
        >
          All ({typeCounts.all})
        </button>
        <button 
          onClick={() => setFilter('slack_thread')}
          className={`filter-chip ${filter === 'slack_thread' ? 'active' : ''}`}
        >
          Slack ({typeCounts.slack_thread})
        </button>
        <button 
          onClick={() => setFilter('github_action')}
          className={`filter-chip ${filter === 'github_action' ? 'active' : ''}`}
        >
          Actions ({typeCounts.github_action})
        </button>
        <button 
          onClick={() => setFilter('github_pr')}
          className={`filter-chip ${filter === 'github_pr' ? 'active' : ''}`}
        >
          PRs ({typeCounts.github_pr})
        </button>
        <button 
          onClick={() => setFilter('copilot_agent')}
          className={`filter-chip ${filter === 'copilot_agent' ? 'active' : ''}`}
        >
          Copilot ({typeCounts.copilot_agent})
        </button>
        <button 
          onClick={() => setFilter('cli_session')}
          className={`filter-chip ${filter === 'cli_session' ? 'active' : ''}`}
        >
          CLI ({typeCounts.cli_session})
        </button>
        <button 
          onClick={() => setFilter('opencode_session')}
          className={`filter-chip ${filter === 'opencode_session' ? 'active' : ''}`}
        >
          OpenCode ({typeCounts.opencode_session})
        </button>
      </div>

      {sortedItems.length === 0 ? (
        <div className="empty-state">
          {items.length === 0 
            ? 'No items tracked yet. Add a URL above to get started!' 
            : 'No items in this category'}
        </div>
      ) : (
        sortedItems.map(item => (
          <ItemCard key={item.id} item={item} onRemove={handleRemove} />
        ))
      )}
    </div>
  );
}
