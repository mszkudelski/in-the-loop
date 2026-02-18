import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Credentials, Settings as SettingsType } from '../types';

export function Settings() {
  const [slackToken, setSlackToken] = useState('');
  const [githubToken, setGithubToken] = useState('');
  const [opencodeUrl, setOpencodeUrl] = useState('');
  const [opencodePassword, setOpencodePassword] = useState('');
  const [pollingInterval, setPollingInterval] = useState(30);
  const [loading, setLoading] = useState(false);
  const [message, setMessage] = useState('');

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const settings: SettingsType = await invoke('get_settings');
      setPollingInterval(settings.polling_interval);
    } catch (error) {
      console.error('Failed to load settings:', error);
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setMessage('');

    try {
      // Save credentials
      const credentials: Credentials = {};
      if (slackToken) credentials.slack_token = slackToken;
      if (githubToken) credentials.github_token = githubToken;
      if (opencodeUrl) credentials.opencode_url = opencodeUrl;
      if (opencodePassword) credentials.opencode_password = opencodePassword;
      
      await invoke('save_credentials', { credentials });
      
      // Save settings
      await invoke('save_settings', { 
        settings: { polling_interval: pollingInterval } 
      });
      
      setMessage('Settings saved successfully!');
      // Clear token fields after save
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
    <div className="settings-panel">
      <h2>Settings</h2>
      <form onSubmit={handleSave}>
        <div className="settings-field">
          <label htmlFor="slack-token">Slack User Token (xoxp-...)</label>
          <input
            id="slack-token"
            type="password"
            className="form-input"
            placeholder="Enter new token or leave blank to keep existing"
            value={slackToken}
            onChange={(e) => setSlackToken(e.target.value)}
          />
        </div>

        <div className="settings-field">
          <label htmlFor="github-token">GitHub Personal Access Token</label>
          <input
            id="github-token"
            type="password"
            className="form-input"
            placeholder="Enter new token or leave blank to keep existing"
            value={githubToken}
            onChange={(e) => setGithubToken(e.target.value)}
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
          <label htmlFor="opencode-password">OpenCode Server Password</label>
          <input
            id="opencode-password"
            type="password"
            className="form-input"
            placeholder="Leave blank if no password set"
            value={opencodePassword}
            onChange={(e) => setOpencodePassword(e.target.value)}
          />
        </div>

        <div className="settings-field">
          <label htmlFor="polling-interval">
            Polling Interval (seconds): {pollingInterval}s
          </label>
          <input
            id="polling-interval"
            type="range"
            min="15"
            max="300"
            step="15"
            value={pollingInterval}
            onChange={(e) => setPollingInterval(parseInt(e.target.value))}
            style={{ width: '100%' }}
          />
          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.875rem', color: '#999' }}>
            <span>15s</span>
            <span>1min</span>
            <span>5min</span>
          </div>
        </div>

        {message && (
          <div style={{ 
            padding: '12px', 
            marginBottom: '12px', 
            borderRadius: '4px',
            background: message.includes('Error') ? '#7f1d1d' : '#065f46',
            color: message.includes('Error') ? '#fca5a5' : '#86efac'
          }}>
            {message}
          </div>
        )}

        <button type="submit" disabled={loading}>
          {loading ? 'Saving...' : 'Save Settings'}
        </button>
      </form>
    </div>
  );
}
