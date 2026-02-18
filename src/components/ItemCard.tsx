import { Item } from '../types';
import { StatusBadge } from './StatusBadge';
import { invoke } from '@tauri-apps/api/core';

interface ItemCardProps {
  item: Item;
  onRemove: (id: string) => void;
}

export function ItemCard({ item, onRemove }: ItemCardProps) {
  const typeName: Record<Item['type'], string> = {
    slack_thread: 'Slack',
    github_action: 'GitHub Action',
    github_pr: 'PR',
    copilot_agent: 'Copilot Agent',
    cli_session: 'CLI Session',
    opencode_session: 'OpenCode',
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

  const formatEpochMs = (epochMs?: number) => {
    if (!epochMs) return null;
    const date = new Date(epochMs);
    return date.toLocaleString();
  };

  const lastLlmResponse = item.type === 'opencode_session'
    ? formatEpochMs(item.metadata?.last_activity)
    : null;

  return (
    <div className="item-card">
      <div className="item-header">
        <span className="type-badge">
          {typeName[item.type]}
        </span>
        {item.url ? (
          <button className="item-title-link" onClick={handleOpen}>
            {item.title}
          </button>
        ) : (
          <h3 className="item-title">{item.title}</h3>
        )}
        <StatusBadge status={item.status} />
      </div>
      <div className="item-meta">
        {lastLlmResponse && (
          <div>Last LLM response: {lastLlmResponse}</div>
        )}
        <div>Last checked: {formatDate(item.last_checked_at)}</div>
      </div>
      <div className="item-actions">
        <button className="btn-ghost" onClick={handleRemove}>Remove</button>
      </div>
    </div>
  );
}
