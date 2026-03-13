import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Credentials, Settings as SettingsType } from '../types';

export function Settings() {
  const [slackToken, setSlackToken] = useState('');
  const [githubToken, setGithubToken] = useState('');
  const [opencodeUrl, setOpencodeUrl] = useState('');
  const [opencodePassword, setOpencodePassword] = useState('');
  const [pollingInterval, setPollingInterval] = useState(30);
  const [notifySessionStarted, setNotifySessionStarted] = useState(true);
  const [notifySessionEnded, setNotifySessionEnded] = useState(true);
  const [notifyInputNeeded, setNotifyInputNeeded] = useState(true);
  const [githubUsername, setGithubUsername] = useState('');
  const [addItemShortcut, setAddItemShortcut] = useState('Ctrl+Shift+Q');
  const [recordingShortcut, setRecordingShortcut] = useState(false);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const settings: SettingsType = await invoke('get_settings');
      setPollingInterval(settings.polling_interval);
      setNotifySessionStarted(settings.notify_session_started);
      setNotifySessionEnded(settings.notify_session_ended);
      setNotifyInputNeeded(settings.notify_input_needed);
      setGithubUsername(settings.github_username || '');

      const shortcut: string = await invoke('get_add_item_shortcut');
      setAddItemShortcut(shortcut);
    } catch (error) {
      console.error('Failed to load settings:', error);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setMessage('');

    try {
      const credentials: Credentials = {};
      if (slackToken) credentials.slack_token = slackToken;
      if (githubToken) credentials.github_token = githubToken;
      if (opencodeUrl) credentials.opencode_url = opencodeUrl;
      if (opencodePassword) credentials.opencode_password = opencodePassword;
      
      await invoke('save_credentials', { credentials });
      await invoke('save_settings', { 
        settings: {
          polling_interval: pollingInterval,
          notify_session_started: notifySessionStarted,
          notify_session_ended: notifySessionEnded,
          notify_input_needed: notifyInputNeeded,
          github_username: githubUsername,
        } 
      });

      await invoke('update_add_item_shortcut', { shortcutStr: addItemShortcut });
      
      setMessage('Saved');
      setSlackToken('');
      setGithubToken('');
      setOpencodeUrl('');
      setOpencodePassword('');
    } catch (error) {
      setMessage(`Error: ${error}`);
    } finally {
      setLoading(false);
    }
  };

  return (
    <form className="settings-form" onSubmit={handleSave}>
      <div className="settings-field">
        <label htmlFor="github-token">GitHub Token</label>
        <input
          id="github-token"
          type="password"
          className="form-input"
          placeholder="ghp_..."
          value={githubToken}
          onChange={(e) => setGithubToken(e.target.value)}
        />
      </div>

      <div className="settings-field">
        <label htmlFor="github-username">GitHub Username</label>
        <input
          id="github-username"
          type="text"
          className="form-input"
          placeholder="Your GitHub login (for filtering PRs)"
          value={githubUsername}
          onChange={(e) => setGithubUsername(e.target.value)}
        />
      </div>

      <div className="settings-field">
        <label htmlFor="opencode-url">OpenCode URL</label>
        <input
          id="opencode-url"
          type="text"
          className="form-input"
          placeholder="Paste any OpenCode URL from your browser"
          value={opencodeUrl}
          onChange={(e) => setOpencodeUrl(e.target.value)}
        />
      </div>

      <div className="settings-field">
        <label htmlFor="opencode-password">OpenCode Password</label>
        <input
          id="opencode-password"
          type="password"
          className="form-input"
          placeholder="Leave blank if none"
          value={opencodePassword}
          onChange={(e) => setOpencodePassword(e.target.value)}
        />
      </div>

      <div className="settings-field">
        <label htmlFor="polling-interval">
          Polling: {pollingInterval}s
        </label>
        <input
          id="polling-interval"
          type="range"
          min="5"
          max="300"
          step="5"
          value={pollingInterval}
          onChange={(e) => setPollingInterval(parseInt(e.target.value))}
          style={{ width: '100%' }}
        />
        <div className="range-labels">
          <span>5s</span>
          <span>1min</span>
          <span>5min</span>
        </div>
      </div>

      <div className="settings-field">
        <label htmlFor="add-item-shortcut">
          Add Item Shortcut
        </label>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          <input
            id="add-item-shortcut"
            type="text"
            className="form-input"
            value={recordingShortcut ? 'Press shortcut...' : addItemShortcut}
            readOnly
            onKeyDown={(e) => {
              if (!recordingShortcut) return;
              e.preventDefault();
              const key = e.key;
              if (['Shift', 'Control', 'Alt', 'Meta'].includes(key)) return;

              const parts: string[] = [];
              if (e.metaKey) parts.push('Super');
              if (e.ctrlKey) parts.push('Control');
              if (e.altKey) parts.push('Alt');
              if (e.shiftKey) parts.push('Shift');
              parts.push(key.length === 1 ? key.toUpperCase() : key);

              setAddItemShortcut(parts.join('+'));
              setRecordingShortcut(false);
            }}
            onBlur={() => setRecordingShortcut(false)}
            style={{ flex: 1 }}
          />
          <button
            type="button"
            onClick={() => setRecordingShortcut(!recordingShortcut)}
            style={{ whiteSpace: 'nowrap' }}
          >
            {recordingShortcut ? 'Cancel' : 'Record'}
          </button>
        </div>
        <span style={{ fontSize: '0.8em', opacity: 0.7 }}>
          Copy a URL, press this shortcut to add it
        </span>
      </div>

      <div className="settings-field">
        <label>Notifications</label>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={notifySessionStarted}
            onChange={(e) => setNotifySessionStarted(e.target.checked)}
          />
          Session started
        </label>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={notifySessionEnded}
            onChange={(e) => setNotifySessionEnded(e.target.checked)}
          />
          Session ended
        </label>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={notifyInputNeeded}
            onChange={(e) => setNotifyInputNeeded(e.target.checked)}
          />
          Input needed
        </label>
      </div>

      {message && (
        <div className={`settings-msg ${message.includes('Error') ? 'settings-msg-error' : ''}`}>
          {message}
        </div>
      )}

      <button type="submit" disabled={loading}>
        {loading ? 'Saving...' : 'Save'}
      </button>
    </form>
  );
}
