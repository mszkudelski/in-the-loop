import { useState, useEffect, useCallback } from 'react';
import { invoke, Channel } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';

// -----------------------------------------------------------------------
// Types matching the Rust DownloadEvent enum (tag + data)
// -----------------------------------------------------------------------
type DownloadEvent =
  | { event: 'started'; data: { contentLength: number | null } }
  | { event: 'progress'; data: { chunkLength: number } }
  | { event: 'finished'; data: Record<string, never> };

type UpdateState =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'installing'
  | 'done'
  | 'error';

const RECHECK_INTERVAL_MS = 4 * 60 * 60 * 1000; // 4 hours

export function UpdatePrompt() {
  const [state, setState] = useState<UpdateState>('idle');
  const [newVersion, setNewVersion] = useState<string | null>(null);
  const [downloaded, setDownloaded] = useState(0);
  const [total, setTotal] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  // -----------------------------------------------------------------------
  // Check for update
  // -----------------------------------------------------------------------
  const checkForUpdate = useCallback(async () => {
    setState('checking');
    setError(null);
    try {
      const version = await invoke<string | null>('fetch_update');
      if (version) {
        setNewVersion(version);
        setState('available');
        setDismissed(false);
      } else {
        setState('idle');
      }
    } catch (e) {
      // Silently ignore update check errors (network offline, etc.)
      console.warn('[updater] check failed:', e);
      setState('idle');
    }
  }, []);

  // Auto-check on mount and periodically
  useEffect(() => {
    checkForUpdate();
    const timer = setInterval(checkForUpdate, RECHECK_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [checkForUpdate]);

  // -----------------------------------------------------------------------
  // Download + install
  // -----------------------------------------------------------------------
  const handleInstall = async () => {
    setState('downloading');
    setDownloaded(0);
    setTotal(null);
    setError(null);

    const channel = new Channel<DownloadEvent>();
    channel.onmessage = (msg) => {
      if (msg.event === 'started') {
        setTotal(msg.data.contentLength);
      } else if (msg.event === 'progress') {
        setDownloaded((prev) => prev + msg.data.chunkLength);
      } else if (msg.event === 'finished') {
        setState('installing');
      }
    };

    try {
      await invoke('install_update', { onEvent: channel });
      setState('done');
    } catch (e) {
      setError(String(e));
      setState('error');
    }
  };

  const handleRelaunch = () => relaunch();

  // -----------------------------------------------------------------------
  // Render — only show a banner/prompt when actionable
  // -----------------------------------------------------------------------
  if (dismissed || state === 'idle' || state === 'checking') return null;

  if (state === 'error') {
    return (
      <div className="update-banner update-banner--error" role="alert">
        <span>⚠️ Update failed: {error}</span>
        <div className="update-banner__actions">
          <button className="btn-ghost" onClick={checkForUpdate}>Retry</button>
          <button className="btn-icon" onClick={() => setDismissed(true)} title="Dismiss">×</button>
        </div>
      </div>
    );
  }

  if (state === 'done') {
    return (
      <div className="update-banner update-banner--done" role="alert">
        <span>✅ Update installed — restart to apply.</span>
        <div className="update-banner__actions">
          <button className="btn-primary" onClick={handleRelaunch}>Restart now</button>
        </div>
      </div>
    );
  }

  if (state === 'downloading' || state === 'installing') {
    const pct =
      total && total > 0 ? Math.round((downloaded / total) * 100) : null;

    return (
      <div className="update-banner update-banner--progress" role="status">
        <span>
          {state === 'installing'
            ? '⚙️ Installing…'
            : pct !== null
            ? `⬇️ Downloading… ${pct}%`
            : '⬇️ Downloading…'}
        </span>
        <div className="update-progress-bar">
          {pct !== null ? (
            <div className="update-progress-bar__fill" style={{ width: `${pct}%` }} />
          ) : (
            <div className="update-progress-bar__fill update-progress-bar__fill--indeterminate" />
          )}
        </div>
      </div>
    );
  }

  // state === 'available'
  return (
    <div className="update-banner update-banner--available" role="alert">
      <span>
        🆕 <strong>v{newVersion}</strong> is available
      </span>
      <div className="update-banner__actions">
        <button className="btn-primary" onClick={handleInstall}>Update now</button>
        <button className="btn-ghost" onClick={() => setDismissed(true)}>Later</button>
      </div>
    </div>
  );
}
