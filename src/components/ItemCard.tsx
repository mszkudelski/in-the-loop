import { Item } from '../types';
import { StatusBadge } from './StatusBadge';
import { invoke } from '@tauri-apps/api/core';

interface ItemCardProps {
  item: Item;
  onRemove: (id: string) => void;
}

export function ItemCard({ item, onRemove }: ItemCardProps) {
  const typeIcons: Record<Item['type'], string> = {
    slack_thread: '💬',
    github_action: '⚙️',
    github_pr: '🔀',
    copilot_agent: '🤖',
    cli_session: '💻',
  };

  const typeName: Record<Item['type'], string> = {
    slack_thread: 'Slack',
    github_action: 'GitHub Action',
    github_pr: 'PR',
    copilot_agent: 'Copilot Agent',
    cli_session: 'CLI Session',
  };

  const handleOpen = () => {
    if (item.url) {
      invoke('open_url', { url: item.url });
    }
  };

  const handleRemove = async () => {
    try {
      await invoke('remove_item', { id: item.id });
      onRemove(item.id);
    } catch (error) {
      console.error('Failed to remove item:', error);
    }
  };

  const formatDate = (dateStr?: string) => {
    if (!dateStr) return 'Never';
    const date = new Date(dateStr);
    return date.toLocaleString();
  };

  return (
    <div className="item-card">
      <div className="item-header">
        <span className="type-badge">
          {typeIcons[item.type]} {typeName[item.type]}
        </span>
        <h3 className="item-title">{item.title}</h3>
        <StatusBadge status={item.status} />
      </div>
      <div style={{ textAlign: 'left', fontSize: '0.875rem', color: '#999' }}>
        Last checked: {formatDate(item.last_checked_at)}
      </div>
      <div className="item-actions">
        {item.url && (
          <button onClick={handleOpen}>Open →</button>
        )}
        <button onClick={handleRemove}>🗑️ Remove</button>
      </div>
    </div>
  );
}
