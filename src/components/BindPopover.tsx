import { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Item, TodoWithBindings } from '../types';

interface BindPopoverProps {
  x: number;
  y: number;
  mode: 'items' | 'todos';
  /** When mode='items', this is the todo_id. When mode='todos', this is the item_id. */
  sourceId: string;
  boundIds: string[];
  onClose: () => void;
  onChanged: () => void;
}

function TodoOptionRow({ todo, bound, onToggle, indent }: {
  todo: TodoWithBindings;
  bound: Set<string>;
  onToggle: (id: string) => void;
  indent: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const hasSubtasks = (todo.subtasks || []).length > 0;

  return (
    <>
      <label className={`bind-popover-option ${indent ? 'bind-popover-indent' : ''}`}>
        {!indent && hasSubtasks && (
          <button
            type="button"
            className="btn-expand"
            onClick={e => { e.preventDefault(); setExpanded(!expanded); }}
          >
            {expanded ? '▾' : '▸'}
          </button>
        )}
        {!indent && !hasSubtasks && <span className="btn-expand-placeholder" />}
        <input
          type="checkbox"
          checked={bound.has(todo.id)}
          onChange={() => onToggle(todo.id)}
        />
        <span className="bind-popover-label">{todo.title}</span>
      </label>
      {expanded && (todo.subtasks || []).map(sub => (
        <TodoOptionRow key={sub.id} todo={sub} bound={bound} onToggle={onToggle} indent />
      ))}
    </>
  );
}

export function BindPopover({ x, y, mode, sourceId, boundIds, onClose, onChanged }: BindPopoverProps) {
  const [itemOptions, setItemOptions] = useState<{ id: string; label: string }[]>([]);
  const [todoOptions, setTodoOptions] = useState<TodoWithBindings[]>([]);
  const [bound, setBound] = useState<Set<string>>(new Set(boundIds));
  const popoverRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadOptions();
  }, []);

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', handleClick);
    document.addEventListener('keydown', handleEsc);
    return () => {
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleEsc);
    };
  }, [onClose]);

  const loadOptions = async () => {
    try {
      if (mode === 'items') {
        const items: Item[] = await invoke('get_items', { archived: false });
        setItemOptions(items.map(i => ({
          id: i.id,
          label: `${i.title}`,
        })));
      } else {
        const todos: TodoWithBindings[] = await invoke('get_todos');
        setTodoOptions(todos);
      }
    } catch (error) {
      console.error('Failed to load options:', error);
    }
  };

  const handleToggle = async (optionId: string) => {
    const isBound = bound.has(optionId);
    const todoId = mode === 'items' ? sourceId : optionId;
    const itemId = mode === 'items' ? optionId : sourceId;

    try {
      if (isBound) {
        await invoke('unbind_todo_from_item', { todoId, itemId });
        setBound(prev => {
          const next = new Set(prev);
          next.delete(optionId);
          return next;
        });
      } else {
        await invoke('bind_todo_to_item', { todoId, itemId });
        setBound(prev => new Set(prev).add(optionId));
      }
      onChanged();
    } catch (error) {
      console.error('Failed to toggle binding:', error);
    }
  };

  const hasOptions = mode === 'items' ? itemOptions.length > 0 : todoOptions.length > 0;

  return (
    <div
      ref={popoverRef}
      className="bind-popover"
      style={{ top: y, left: x }}
    >
      <div className="bind-popover-title">
        {mode === 'items' ? 'Bind to item' : 'Bind to todo'}
      </div>
      {!hasOptions ? (
        <div className="bind-popover-empty">
          {mode === 'items' ? 'No items available' : 'No todos available'}
        </div>
      ) : (
        <div className="bind-popover-list">
          {mode === 'items' ? (
            itemOptions.map(opt => (
              <label key={opt.id} className="bind-popover-option">
                <input
                  type="checkbox"
                  checked={bound.has(opt.id)}
                  onChange={() => handleToggle(opt.id)}
                />
                <span className="bind-popover-label">{opt.label}</span>
              </label>
            ))
          ) : (
            todoOptions.map(todo => (
              <TodoOptionRow
                key={todo.id}
                todo={todo}
                bound={bound}
                onToggle={handleToggle}
                indent={false}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
}
