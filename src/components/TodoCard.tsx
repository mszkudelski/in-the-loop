import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TodoWithBindings } from '../types';
import { StatusBadge } from './StatusBadge';
import { ContextMenu } from './ContextMenu';
import { BindPopover } from './BindPopover';

interface TodoCardProps {
  todo: TodoWithBindings;
  onChanged: () => void;
}

export function TodoCard({ todo, onChanged }: TodoCardProps) {
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [bindPopover, setBindPopover] = useState<{ x: number; y: number } | null>(null);

  const handleToggleStatus = async () => {
    const newStatus = todo.status === 'open' ? 'done' : 'open';
    try {
      await invoke('update_todo_status', { id: todo.id, status: newStatus });
      onChanged();
    } catch (error) {
      console.error('Failed to update todo status:', error);
    }
  };

  const handleDelete = async () => {
    try {
      await invoke('delete_todo', { id: todo.id });
      onChanged();
    } catch (error) {
      console.error('Failed to delete todo:', error);
    }
  };

  const handleUnbind = async (itemId: string) => {
    try {
      await invoke('unbind_todo_from_item', { todoId: todo.id, itemId });
      onChanged();
    } catch (error) {
      console.error('Failed to unbind item:', error);
    }
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY });
  };

  const handleBindClick = () => {
    if (contextMenu) {
      setBindPopover({ x: contextMenu.x, y: contextMenu.y });
      setContextMenu(null);
    }
  };

  const typeName: Record<string, string> = {
    slack_thread: 'Slack',
    github_action: 'Action',
    github_pr: 'PR',
    copilot_agent: 'Copilot',
    cli_session: 'CLI',
    opencode_session: 'OpenCode',
  };

  return (
    <>
      <div
        className={`todo-card ${todo.status === 'done' ? 'item-checked' : ''}`}
        onContextMenu={handleContextMenu}
      >
        <div className="item-row">
          <input
            type="checkbox"
            className="item-checkbox"
            checked={todo.status === 'done'}
            onChange={handleToggleStatus}
          />
          <span className={`item-title ${todo.status === 'done' ? 'todo-done-text' : ''}`}>
            {todo.title}
          </span>
          {todo.bound_items.length > 0 && (
            <span className="todo-binding-count">
              🔗 {todo.bound_items.length}
            </span>
          )}
          <button className="btn-icon" onClick={handleDelete} title="Delete">✕</button>
        </div>
        {todo.bound_items.length > 0 && (
          <div className="todo-bound-items">
            {todo.bound_items.map(item => (
              <div key={item.id} className="todo-bound-item">
                <span className="type-badge">{typeName[item.type] || item.type}</span>
                <StatusBadge status={item.status} />
                <span className="todo-bound-item-title">{item.title}</span>
                <button
                  className="btn-icon btn-unbind"
                  onClick={() => handleUnbind(item.id)}
                  title="Unbind"
                >✕</button>
              </div>
            ))}
          </div>
        )}
      </div>

      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={[
            { label: 'Bind to item...', onClick: handleBindClick },
            { label: todo.status === 'open' ? 'Mark done' : 'Reopen', onClick: () => { handleToggleStatus(); setContextMenu(null); } },
            { label: 'Delete', onClick: () => { handleDelete(); setContextMenu(null); } },
          ]}
          onClose={() => setContextMenu(null)}
        />
      )}

      {bindPopover && (
        <BindPopover
          x={bindPopover.x}
          y={bindPopover.y}
          mode="items"
          sourceId={todo.id}
          boundIds={todo.bound_items.map(i => i.id)}
          onClose={() => setBindPopover(null)}
          onChanged={onChanged}
        />
      )}
    </>
  );
}
