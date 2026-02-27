import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Todo } from '../types';

interface AddTodoFormProps {
  onTodoAdded: () => void;
}

export function AddTodoForm({ onTodoAdded }: AddTodoFormProps) {
  const [title, setTitle] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = title.trim();
    if (!trimmed) return;

    try {
      await invoke<Todo>('add_todo', { title: trimmed });
      setTitle('');
      onTodoAdded();
    } catch (error) {
      console.error('Failed to add todo:', error);
    }
  };

  return (
    <form className="add-form-inline" onSubmit={handleSubmit}>
      <input
        className="form-input"
        type="text"
        placeholder="Add a todo..."
        value={title}
        onChange={e => setTitle(e.target.value)}
      />
      <button type="submit" disabled={!title.trim()}>Add</button>
    </form>
  );
}
