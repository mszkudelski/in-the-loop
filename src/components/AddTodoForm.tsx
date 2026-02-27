import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Todo } from '../types';

interface AddTodoFormProps {
  onTodoAdded: () => void;
  parentId?: string;
  compact?: boolean;
}

export function AddTodoForm({ onTodoAdded, parentId, compact }: AddTodoFormProps) {
  const [title, setTitle] = useState('');
  const [plannedDate, setPlannedDate] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = title.trim();
    if (!trimmed) return;

    try {
      await invoke<Todo>('add_todo', {
        title: trimmed,
        plannedDate: plannedDate || null,
        parentId: parentId || null,
      });
      setTitle('');
      setPlannedDate('');
      onTodoAdded();
    } catch (error) {
      console.error('Failed to add todo:', error);
    }
  };

  return (
    <form className={`add-form-inline ${compact ? 'add-form-compact' : ''}`} onSubmit={handleSubmit}>
      <input
        className="form-input"
        type="text"
        placeholder={parentId ? 'Add a subtask...' : 'Add a todo...'}
        value={title}
        onChange={e => setTitle(e.target.value)}
      />
      {!compact && (
        <input
          className="form-input date-input"
          type="date"
          value={plannedDate}
          onChange={e => setPlannedDate(e.target.value)}
          title="Planned date"
        />
      )}
      <button type="submit" disabled={!title.trim()}>Add</button>
    </form>
  );
}
