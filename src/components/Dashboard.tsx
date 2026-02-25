import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Item } from '../types';
import { ItemCard } from './ItemCard';
import { AddItemForm } from './AddItemForm';
import { Settings } from './Settings';
import { UpdatePrompt } from './UpdatePrompt';

export function Dashboard() {
  const [items, setItems] = useState<Item[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [filter, setFilter] = useState<Item['type'] | 'all'>('all');
  const [hideChecked, setHideChecked] = useState(false);

  useEffect(() => {
    loadItems();
    invoke<string | null>('get_setting', { key: 'hide_checked' }).then(val => {
      if (val === 'true') setHideChecked(true);
    }).catch(() => {});

    const unlisten = listen('item-updated', () => {
      loadItems();
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const loadItems = async () => {
    try {
      const loadedItems: Item[] = await invoke('get_items', { archived: false });
      const parsedItems = loadedItems.map(item => ({
        ...item,
        metadata: typeof item.metadata === 'string' ? JSON.parse(item.metadata) : item.metadata,
      }));
      setItems(parsedItems);
    } catch (error) {
      console.error('Failed to load items:', error);
    }
  };

  const handleRemove = (id: string) => {
    setItems(items.filter(item => item.id !== id));
  };

  const handleToggleChecked = async (id: string, checked: boolean) => {
    try {
      await invoke('toggle_checked', { id, checked });
      setItems(prev => prev.map(item =>
        item.id === id ? { ...item, checked } : item
      ));
    } catch (error) {
      console.error('Failed to toggle checked:', error);
    }
  };

  const filteredItems = (filter === 'all' 
    ? items 
    : items.filter(item => item.type === filter)
  ).filter(item => !hideChecked || !item.checked);

  const sortedItems = [...filteredItems].sort((a, b) => {
    if (a.checked && !b.checked) return 1;
    if (!a.checked && b.checked) return -1;
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
      <div className="header-row">
        <h1 className="page-title">In The Loop</h1>
        <div className="header-actions">
          <button className="btn-ghost" onClick={loadItems}>Refresh</button>
          <button className="btn-ghost" onClick={() => setShowSettings(true)}>Settings</button>
        </div>
      </div>

      <UpdatePrompt />
      <AddItemForm onItemAdded={loadItems} />

      <div className="filter-row">
        <button 
          onClick={() => setFilter('all')}
          className={`filter-chip ${filter === 'all' ? 'active' : ''}`}
        >
          All ({typeCounts.all})
        </button>
        {typeCounts.slack_thread > 0 && (
          <button 
            onClick={() => setFilter('slack_thread')}
            className={`filter-chip ${filter === 'slack_thread' ? 'active' : ''}`}
          >
            Slack ({typeCounts.slack_thread})
          </button>
        )}
        {typeCounts.github_action > 0 && (
          <button 
            onClick={() => setFilter('github_action')}
            className={`filter-chip ${filter === 'github_action' ? 'active' : ''}`}
          >
            Actions ({typeCounts.github_action})
          </button>
        )}
        {typeCounts.github_pr > 0 && (
          <button 
            onClick={() => setFilter('github_pr')}
            className={`filter-chip ${filter === 'github_pr' ? 'active' : ''}`}
          >
            PRs ({typeCounts.github_pr})
          </button>
        )}
        {typeCounts.copilot_agent > 0 && (
          <button 
            onClick={() => setFilter('copilot_agent')}
            className={`filter-chip ${filter === 'copilot_agent' ? 'active' : ''}`}
          >
            Copilot ({typeCounts.copilot_agent})
          </button>
        )}
        {typeCounts.cli_session > 0 && (
          <button 
            onClick={() => setFilter('cli_session')}
            className={`filter-chip ${filter === 'cli_session' ? 'active' : ''}`}
          >
            CLI ({typeCounts.cli_session})
          </button>
        )}
        {typeCounts.opencode_session > 0 && (
          <button 
            onClick={() => setFilter('opencode_session')}
            className={`filter-chip ${filter === 'opencode_session' ? 'active' : ''}`}
          >
            OpenCode ({typeCounts.opencode_session})
          </button>
        )}
        <label className="filter-toggle">
          <input
            type="checkbox"
            checked={hideChecked}
            onChange={() => {
              const next = !hideChecked;
              setHideChecked(next);
              invoke('save_setting', { key: 'hide_checked', value: String(next) }).catch(() => {});
            }}
          />
          Hide checked
        </label>
      </div>

      {sortedItems.length === 0 ? (
        <div className="empty-state">
          {items.length === 0 
            ? 'No items tracked yet. Add a URL above to get started.' 
            : 'No items in this category'}
        </div>
      ) : (
        <div className="item-list">
          {sortedItems.map(item => (
            <ItemCard
              key={item.id}
              item={item}
              onRemove={handleRemove}
              onToggleChecked={handleToggleChecked}
            />
          ))}
        </div>
      )}

      {showSettings && (
        <div className="modal-backdrop" onClick={() => setShowSettings(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <div className="modal-header">
              <h2>Settings</h2>
              <button className="btn-icon" onClick={() => setShowSettings(false)}>&times;</button>
            </div>
            <Settings />
          </div>
        </div>
      )}
    </div>
  );
}
