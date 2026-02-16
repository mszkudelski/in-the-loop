import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AddItemFormProps {
  onItemAdded: () => void;
}

export function AddItemForm({ onItemAdded }: AddItemFormProps) {
  const [url, setUrl] = useState('');
  const [customTitle, setCustomTitle] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      await invoke('add_item', { 
        url, 
        customTitle: customTitle || undefined 
      });
      setUrl('');
      setCustomTitle('');
      onItemAdded();
    } catch (err) {
      setError(err as string);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="add-form">
      <h2>Add Item to Track</h2>
      <form onSubmit={handleSubmit}>
        <input
          type="text"
          className="form-input"
          placeholder="Paste URL (Slack thread, GitHub Action, PR, etc.)"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          required
        />
        <input
          type="text"
          className="form-input"
          placeholder="Custom title (optional)"
          value={customTitle}
          onChange={(e) => setCustomTitle(e.target.value)}
        />
        {error && (
          <div style={{ color: '#ef4444', marginBottom: '12px' }}>
            {error}
          </div>
        )}
        <button type="submit" disabled={loading}>
          {loading ? 'Adding...' : 'Add Item'}
        </button>
      </form>
    </div>
  );
}
