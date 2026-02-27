import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { TodoWithBindings } from '../types';
import { TodoCard } from './TodoCard';
import { AddTodoForm } from './AddTodoForm';

export function TodoList() {
  const [todos, setTodos] = useState<TodoWithBindings[]>([]);

  useEffect(() => {
    loadTodos();
  }, []);

  const loadTodos = async () => {
    try {
      const loaded: TodoWithBindings[] = await invoke('get_todos');
      const parsed = loaded.map(t => ({
        ...t,
        bound_items: t.bound_items.map(item => ({
          ...item,
          metadata: typeof item.metadata === 'string' ? JSON.parse(item.metadata) : item.metadata,
        })),
      }));
      setTodos(parsed);
    } catch (error) {
      console.error('Failed to load todos:', error);
    }
  };

  const openTodos = todos.filter(t => t.status === 'open');
  const doneTodos = todos.filter(t => t.status === 'done');

  return (
    <>
      <div className="header-row">
        <h1 className="page-title">Todos</h1>
        <div className="header-actions">
          <button className="btn-ghost" onClick={loadTodos}>Refresh</button>
        </div>
      </div>

      <AddTodoForm onTodoAdded={loadTodos} />

      {todos.length === 0 ? (
        <div className="empty-state">
          No todos yet. Add one above to get started.
        </div>
      ) : (
        <>
          <div className="item-list">
            {openTodos.map(todo => (
              <TodoCard key={todo.id} todo={todo} onChanged={loadTodos} />
            ))}
          </div>
          {doneTodos.length > 0 && (
            <>
              <div className="todo-done-header">Done ({doneTodos.length})</div>
              <div className="item-list">
                {doneTodos.map(todo => (
                  <TodoCard key={todo.id} todo={todo} onChanged={loadTodos} />
                ))}
              </div>
            </>
          )}
        </>
      )}
    </>
  );
}
