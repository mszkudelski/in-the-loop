import { useState } from 'react';
import { Item } from '../types';
import { StatusBadge } from './StatusBadge';
import { invoke } from '@tauri-apps/api/core';
import { ContextMenu } from './ContextMenu';
import { BindPopover } from './BindPopover';

interface ItemCardProps {
  item: Item;
  isArchived: boolean;
  onArchive: (id: string) => void;
  onUnarchive: (id: string) => void;
  githubUsername?: string;
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

export function ItemCard({ item, isArchived, onArchive, onUnarchive, githubUsername }: ItemCardProps) {
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [bindPopover, setBindPopover] = useState<{ x: number; y: number } | null>(null);
  const [boundTodoIds, setBoundTodoIds] = useState<string[]>([]);
  const [expanded, setExpanded] = useState(false);

  const typeName: Record<Item['type'], string> = {
    slack_thread: 'Slack',
    github_action: 'Action',
    github_pr: 'PR',
    github_repo: 'Repo',
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

  const handleContextMenu = async (e: React.MouseEvent) => {
    e.preventDefault();
    try {
      const ids: string[] = await invoke('get_todo_ids_for_item', { itemId: item.id });
      setBoundTodoIds(ids);
    } catch {
      setBoundTodoIds([]);
    }
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const handleBindClick = () => {
    if (contextMenu) {
      setBindPopover({ x: contextMenu.x, y: contextMenu.y });
      setContextMenu(null);
    }
  };

  const lastActivity = getLastActivity(item);
  const lastActivityStr = timeAgo(lastActivity);
  const hasLink = !!(getOpenCodeSessionUrl(item) || item.url);

  const repoDetail = item.type === 'github_repo' && item.metadata?.open_pr_count != null
    ? `${item.metadata.open_pr_count} open PR${item.metadata.open_pr_count === 1 ? '' : 's'}`
    : null;

  const repoPrs = item.type === 'github_repo' && Array.isArray(item.metadata?.open_prs)
    ? item.metadata.open_prs as Array<{ number: number; title: string; author: string; draft: boolean; updated_at: string }>
    : [];

  const myPrs = githubUsername
    ? repoPrs.filter(pr => pr.author.toLowerCase() === githubUsername.toLowerCase())
    : [];
  const otherPrs = githubUsername
    ? repoPrs.filter(pr => pr.author.toLowerCase() !== githubUsername.toLowerCase())
    : repoPrs;

  const hasRepoPrs = repoPrs.length > 0;
  const owner = item.metadata?.owner;
  const repo = item.metadata?.repo;

  return (
    <>
      <div className="item-row" onContextMenu={handleContextMenu}>
        <span className="type-badge">{typeName[item.type]}</span>
        <StatusBadge status={item.status} />
        {hasLink ? (
          <span className="item-title item-title-link" role="link" tabIndex={0} onClick={handleOpen} onKeyDown={e => e.key === 'Enter' && handleOpen()}>
            {item.title}
          </span>
        ) : (
          <span className="item-title">{item.title}</span>
        )}
        {repoDetail && (
          <span className="item-detail">{repoDetail}</span>
        )}
        {hasRepoPrs && (
          <button
            className="btn-icon btn-expand"
            onClick={() => setExpanded(!expanded)}
            title={expanded ? 'Collapse' : 'Expand PRs'}
          >
            {expanded ? '▲' : '▼'}
          </button>
        )}
        {lastActivityStr && (
          <span className="item-time">{lastActivityStr}</span>
        )}
        {isArchived ? (
          <button className="btn-icon" onClick={() => onUnarchive(item.id)} title="Restore">↩</button>
        ) : (
          !hasRepoPrs && <button className="btn-icon" onClick={() => onArchive(item.id)} title="Archive">▼</button>
        )}
      </div>

      {expanded && hasRepoPrs && (
        <div className="repo-pr-panel">
          {myPrs.length > 0 && (
            <div className="repo-pr-section">
              <div className="repo-pr-section-header">My PRs ({myPrs.length})</div>
              {myPrs.map(pr => (
                <div key={pr.number} className="repo-pr-row repo-pr-mine">
                  <span className="repo-pr-number" onClick={() => invoke('open_url', { url: `https://github.com/${owner}/${repo}/pull/${pr.number}` })}>
                    #{pr.number}
                  </span>
                  <span className="repo-pr-title" onClick={() => invoke('open_url', { url: `https://github.com/${owner}/${repo}/pull/${pr.number}` })}>
                    {pr.title}
                  </span>
                  {pr.draft && <span className="repo-pr-draft">draft</span>}
                  <span className="repo-pr-time">{timeAgo(pr.updated_at)}</span>
                </div>
              ))}
            </div>
          )}
          {otherPrs.length > 0 && (
            <div className="repo-pr-section">
              {githubUsername && <div className="repo-pr-section-header">Others ({otherPrs.length})</div>}
              {otherPrs.map(pr => (
                <div key={pr.number} className="repo-pr-row">
                  <span className="repo-pr-number" onClick={() => invoke('open_url', { url: `https://github.com/${owner}/${repo}/pull/${pr.number}` })}>
                    #{pr.number}
                  </span>
                  <span className="repo-pr-title" onClick={() => invoke('open_url', { url: `https://github.com/${owner}/${repo}/pull/${pr.number}` })}>
                    {pr.title}
                  </span>
                  <span className="repo-pr-author">{pr.author}</span>
                  {pr.draft && <span className="repo-pr-draft">draft</span>}
                  <span className="repo-pr-time">{timeAgo(pr.updated_at)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={[
            { label: 'Bind to todo...', onClick: handleBindClick },
          ]}
          onClose={() => setContextMenu(null)}
        />
      )}

      {bindPopover && (
        <BindPopover
          x={bindPopover.x}
          y={bindPopover.y}
          mode="todos"
          sourceId={item.id}
          boundIds={boundTodoIds}
          onClose={() => setBindPopover(null)}
          onChanged={() => {}}
        />
      )}
    </>
  );
}
