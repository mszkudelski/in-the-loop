use crate::db::{Credentials, Database, Item, Settings};
use crate::services::url_parser;
use crate::tray;
use anyhow::Result;
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

pub struct AppState {
    pub db: Arc<Database>,
}

#[tauri::command]
pub async fn add_item(
    url: String,
    custom_title: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let parsed = url_parser::parse_url(&url).map_err(|e| e.to_string())?;

    let item = Item {
        id: Uuid::new_v4().to_string(),
        item_type: parsed.item_type,
        title: custom_title.unwrap_or(parsed.suggested_title),
        url: Some(url),
        status: "waiting".to_string(),
        previous_status: None,
        metadata: serde_json::to_string(&parsed.metadata).map_err(|e| e.to_string())?,
        last_checked_at: None,
        last_updated_at: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        archived: false,
        archived_at: None,
        polling_interval_override: None,
        checked: false,
    };

    state.db.add_item(&item).map_err(|e| e.to_string())?;
    tray::refresh_tray(&app, &state.db);
    Ok(())
}

#[tauri::command]
pub async fn get_items(archived: bool, state: State<'_, AppState>) -> Result<Vec<Item>, String> {
    state.db.get_items(archived).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_item(id: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.db.remove_item(&id).map_err(|e| e.to_string())?;
    tray::refresh_tray(&app, &state.db);
    Ok(())
}

#[tauri::command]
pub async fn archive_item(id: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.db.archive_item(&id).map_err(|e| e.to_string())?;
    tray::refresh_tray(&app, &state.db);
    Ok(())
}

#[tauri::command]
pub async fn archive_items(ids: Vec<String>, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.db.archive_items(&ids).map_err(|e| e.to_string())?;
    tray::refresh_tray(&app, &state.db);
    Ok(())
}

#[tauri::command]
pub async fn unarchive_item(id: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.db.unarchive_item(&id).map_err(|e| e.to_string())?;
    tray::refresh_tray(&app, &state.db);
    Ok(())
}

#[tauri::command]
pub async fn archive_stale_items(before: String, app: AppHandle, state: State<'_, AppState>) -> Result<u64, String> {
    let count = state.db.archive_stale_items(&before).map_err(|e| e.to_string())?;
    tray::refresh_tray(&app, &state.db);
    Ok(count)
}

#[tauri::command]
pub async fn toggle_checked(
    id: String,
    checked: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .db
        .toggle_checked(&id, checked)
        .map_err(|e| e.to_string())?;
    tray::refresh_tray(&app, &state.db);
    Ok(())
}

#[tauri::command]
pub async fn save_credentials(
    credentials: Credentials,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(slack_token) = credentials.slack_token {
        if !slack_token.is_empty() {
            state
                .db
                .save_credential("slack_token", &slack_token)
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(github_token) = credentials.github_token {
        if !github_token.is_empty() {
            state
                .db
                .save_credential("github_token", &github_token)
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(opencode_url) = credentials.opencode_url {
        if !opencode_url.is_empty() {
            state
                .db
                .save_credential("opencode_url", &opencode_url)
                .map_err(|e| e.to_string())?;
        }
    }

    if let Some(opencode_password) = credentials.opencode_password {
        state
            .db
            .save_credential("opencode_password", &opencode_password)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .db
        .save_setting("polling_interval", &settings.polling_interval.to_string())
        .map_err(|e| e.to_string())?;

    state
        .db
        .save_setting("screen_width", &settings.screen_width.to_string())
        .map_err(|e| e.to_string())?;

    if let Some(window) = app.get_webview_window("main") {
        let current_size = window.outer_size().map_err(|e| e.to_string())?;
        let new_size = tauri::PhysicalSize::new(settings.screen_width as u32, current_size.height);
        window.set_size(new_size).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    state.db.get_all_settings().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_setting(
    key: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .db
        .save_setting(&key, &value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_setting(key: String, state: State<'_, AppState>) -> Result<Option<String>, String> {
    state.db.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        // On Linux, use xdg-open
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
