import { Item } from '../types';
import { StatusBadge } from './StatusBadge';
import { invoke } from '@tauri-apps/api/core';

interface ItemCardProps {
  item: Item;
  onRemove: (id: string) => void;
  onToggleChecked: (id: string, checked: boolean) => void;
}

function timeAgo(dateInput?: string | number): string {
  if (!dateInput) return '';
  const date = typeof dateInput === 'number' ? new Date(dateInput) : new Date(dateInput);
  const now = Date.now();
  const diffMs = now - date.getTime();
  if (diffMs < 0) return 'just now';

  const seconds = Math.floor(diffMs / 1000);
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return date.toLocaleDateString();
}

function getOpenCodeSessionUrl(item: Item): string | null {
  if (item.type !== 'opencode_session') return null;
  const baseUrl = item.metadata?.opencode_url;
  const sessionId = item.metadata?.session_id;
  if (!baseUrl || !sessionId) return null;
  return `${baseUrl}/session/${sessionId}`;
}

function getLastActivity(item: Item): string | number | undefined {
  if (item.type === 'opencode_session' || item.type === 'cli_session' || item.type === 'copilot_agent') {
    return item.metadata?.last_activity || undefined;
  }
  return item.last_updated_at || item.last_checked_at;
}

export function ItemCard({ item, onRemove, onToggleChecked }: ItemCardProps) {
  const typeName: Record<Item['type'], string> = {
    slack_thread: 'Slack',
    github_action: 'Action',
    github_pr: 'PR',
    copilot_agent: 'Copilot',
    cli_session: 'CLI',
    opencode_session: 'OpenCode',
  };

  const handleOpen = () => {
    const opencodeUrl = getOpenCodeSessionUrl(item);
    const url = opencodeUrl || item.url;
    if (url) {
      invoke('open_url', { url });
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

  const handleCheck = () => {
    onToggleChecked(item.id, !item.checked);
  };

  const lastActivity = getLastActivity(item);
  const lastActivityStr = timeAgo(lastActivity);
  const hasLink = !!(getOpenCodeSessionUrl(item) || item.url);

  return (
    <div className={`item-row ${item.checked ? 'item-checked' : ''}`}>
      <input
        type="checkbox"
        className="item-checkbox"
        checked={item.checked}
        onChange={handleCheck}
      />
      <span className="type-badge">{typeName[item.type]}</span>
      <StatusBadge status={item.status} />
      {hasLink ? (
        <span className="item-title item-title-link" role="link" tabIndex={0} onClick={handleOpen} onKeyDown={e => e.key === 'Enter' && handleOpen()}>
          {item.title}
        </span>
      ) : (
        <span className="item-title">{item.title}</span>
      )}
      {lastActivityStr && (
        <span className="item-time">{lastActivityStr}</span>
      )}
      <button className="btn-icon" onClick={handleRemove} title="Remove">
        &times;
      </button>
    </div>
  );
}
