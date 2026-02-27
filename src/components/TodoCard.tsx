import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TodoWithBindings } from '../types';
import { StatusBadge } from './StatusBadge';
import { ContextMenu } from './ContextMenu';
import { BindPopover } from './BindPopover';
import { AddTodoForm } from './AddTodoForm';

interface TodoCardProps {
  todo: TodoWithBindings;
  onChanged: () => void;
  isSubtask?: boolean;
}

function formatDate(dateStr: string): string {
  const date = new Date(dateStr + 'T00:00:00');
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const tomorrow = new Date(today);
  tomorrow.setDate(tomorrow.getDate() + 1);
  const dateOnly = new Date(date);
  dateOnly.setHours(0, 0, 0, 0);

  if (dateOnly.getTime() === today.getTime()) return 'Today';
  if (dateOnly.getTime() === tomorrow.getTime()) return 'Tomorrow';

  const isPastDue = dateOnly < today;
  const formatted = date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
  return isPastDue ? `⚠ ${formatted}` : formatted;
}

export function TodoCard({ todo, onChanged, isSubtask }: TodoCardProps) {
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [bindPopover, setBindPopover] = useState<{ x: number; y: number } | null>(null);
  const [showSubtaskForm, setShowSubtaskForm] = useState(false);
  const [editingDate, setEditingDate] = useState(false);

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

  const handleDateChange = async (date: string) => {
    try {
      await invoke('update_todo_date', { id: todo.id, plannedDate: date || null });
      setEditingDate(false);
      onChanged();
    } catch (error) {
      console.error('Failed to update date:', error);
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

  const openSubtasks = (todo.subtasks || []).filter(s => s.status === 'open');
  const doneSubtasks = (todo.subtasks || []).filter(s => s.status === 'done');

  return (
    <>
      <div
        className={`todo-card ${todo.status === 'done' ? 'item-checked' : ''} ${isSubtask ? 'todo-subtask' : ''}`}
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
          {todo.planned_date && !editingDate && (
            <span
              className={`todo-date ${new Date(todo.planned_date + 'T00:00:00') < new Date(new Date().toDateString()) ? 'todo-date-overdue' : ''}`}
              onClick={() => setEditingDate(true)}
              title="Click to change date"
            >
              📅 {formatDate(todo.planned_date)}
            </span>
          )}
          {editingDate && (
            <input
              className="form-input date-input date-input-inline"
              type="date"
              defaultValue={todo.planned_date || ''}
              autoFocus
              onBlur={e => handleDateChange(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter') handleDateChange((e.target as HTMLInputElement).value);
                if (e.key === 'Escape') setEditingDate(false);
              }}
            />
          )}
          {!isSubtask && (todo.subtasks || []).length > 0 && (
            <span className="todo-subtask-count">
              {doneSubtasks.length}/{(todo.subtasks || []).length}
            </span>
          )}
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
        {!isSubtask && (
          <div className="todo-subtasks">
            {openSubtasks.map(subtask => (
              <TodoCard key={subtask.id} todo={subtask} onChanged={onChanged} isSubtask />
            ))}
            {doneSubtasks.length > 0 && openSubtasks.length > 0 && (
              <div className="todo-subtask-done-divider" />
            )}
            {doneSubtasks.map(subtask => (
              <TodoCard key={subtask.id} todo={subtask} onChanged={onChanged} isSubtask />
            ))}
            {showSubtaskForm ? (
              <AddTodoForm parentId={todo.id} onTodoAdded={() => { onChanged(); setShowSubtaskForm(false); }} compact />
            ) : (
              <button className="btn-add-subtask" onClick={() => setShowSubtaskForm(true)}>+ subtask</button>
            )}
          </div>
        )}
      </div>

      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={[
            { label: 'Bind to item...', onClick: handleBindClick },
            ...(!isSubtask ? [{ label: 'Add subtask', onClick: () => { setShowSubtaskForm(true); setContextMenu(null); } }] : []),
            { label: 'Set date...', onClick: () => { setEditingDate(true); setContextMenu(null); } },
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
