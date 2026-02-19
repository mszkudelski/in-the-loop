import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AddItemFormProps {
  onItemAdded: () => void;
}

export function AddItemForm({ onItemAdded }: AddItemFormProps) {
  const [url, setUrl] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      await invoke('add_item', { url, customTitle: undefined });
      setUrl('');
      onItemAdded();
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  return (
    <form className="add-form-inline" onSubmit={handleSubmit}>
      <input
        type="text"
        className="form-input"
        placeholder="Paste a URL to track (Slack, GitHub, OpenCode...)"
        value={url}
        onChange={(e) => setUrl(e.target.value)}
        required
      />
      <button type="submit" disabled={loading}>
        {loading ? '...' : 'Add'}
      </button>
      {error && <span className="form-error-inline">{error}</span>}
    </form>
  );
}
