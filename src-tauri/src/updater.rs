use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, AppHandle, State};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Holds a pending update so the frontend can trigger install separately.
pub struct PendingUpdate(pub Mutex<Option<tauri_plugin_updater::Update>>);

// ---------------------------------------------------------------------------
// Payload types sent over the IPC Channel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum DownloadEvent {
    /// How many bytes the update is in total (may be None if unknown).
    Started {
        content_length: Option<u64>,
    },
    /// Cumulative bytes downloaded so far.
    Progress {
        chunk_length: usize,
    },
    /// Download finished — install will begin immediately.
    Finished,
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Check whether an update is available.
///
/// Returns `Some(version)` if an update exists (and caches it in state),
/// or `None` if the app is already up to date.
#[tauri::command]
pub async fn fetch_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<String>, String> {
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| e.to_string())?;

    let update = updater.check().await.map_err(|e| e.to_string())?;

    match update {
        Some(u) => {
            let version = u.version.clone();
            *pending.0.lock().await = Some(u);
            Ok(Some(version))
        }
        None => Ok(None),
    }
}

/// Download and install the cached pending update.
///
/// Progress events are streamed back via `on_event` Channel.
/// After install the caller should call `plugin:process|relaunch`.
#[tauri::command]
pub async fn install_update(
    pending: State<'_, PendingUpdate>,
    on_event: Channel<DownloadEvent>,
) -> Result<(), String> {
    let mut guard = pending.0.lock().await;
    let update = guard.take().ok_or("No pending update — call fetch_update first")?;

    update
        .download_and_install(
            |chunk_length, content_length| {
                // First callback invocation carries content_length
                let evt = if content_length.is_some() {
                    DownloadEvent::Started { content_length }
                } else {
                    DownloadEvent::Progress { chunk_length }
                };
                // Ignore send errors (window may have been closed)
                let _ = on_event.send(evt);
            },
            || {
                let _ = on_event.send(DownloadEvent::Finished);
            },
        )
        .await
        .map_err(|e| e.to_string())
}
