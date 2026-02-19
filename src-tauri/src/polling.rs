use crate::db::{Database, Item};
use crate::services::{github_actions, github_pr, opencode, slack, url_parser};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;
use tokio::time;

pub struct PollingManager {
    db: Arc<Database>,
    app_handle: AppHandle,
}

impl PollingManager {
    pub fn new(db: Arc<Database>, app_handle: AppHandle) -> Self {
        Self { db, app_handle }
    }

    pub async fn start(&self) {
        let db = self.db.clone();
        let app_handle = self.app_handle.clone();

        tokio::spawn(async move {
            loop {
                // Get polling interval from settings
                let interval = match db.get_setting("polling_interval") {
                    Ok(Some(val)) => val.parse::<u64>().unwrap_or(30),
                    _ => 30,
                };

                // Poll all items
                if let Err(e) = Self::poll_items(&db, &app_handle).await {
                    eprintln!("Error polling items: {}", e);
                }

                time::sleep(Duration::from_secs(interval)).await;
            }
        });
    }

    async fn poll_items(db: &Arc<Database>, app_handle: &AppHandle) -> anyhow::Result<()> {
        if let Err(e) = Self::discover_opencode_sessions(db, app_handle).await {
            eprintln!("Error discovering OpenCode sessions: {}", e);
        }

        let items = db.get_items(false)?;

        let opencode_statuses = Self::get_opencode_context(db).await;

        for item in items {
            // Skip completed/failed items, but keep polling opencode_session
            // (archived sessions need status tracking, idle sessions may become busy)
            if (item.status == "completed" || item.status == "failed" || item.status == "archived")
                && item.item_type != "opencode_session"
            {
                continue;
            }

            let result = match item.item_type.as_str() {
                "slack_thread" => Self::poll_slack_thread(db, &item).await,
                "github_action" => Self::poll_github_action(db, &item).await,
                "github_pr" => Self::poll_github_pr(db, &item).await,
                "opencode_session" => {
                    Self::poll_opencode_session(db, &item, &opencode_statuses, app_handle).await
                }
                _ => continue,
            };

            if let Err(e) = result {
                let error_text = e.to_string();
                let mark_failed = Self::is_permanent_github_error(&item.item_type, &error_text);
                let _ = db.update_item_poll_error(&item.id, &error_text, mark_failed);
                let _ = app_handle.emit("item-updated", &item.id);
                eprintln!("Error polling item {}: {}", item.id, error_text);
            } else {
                // Emit event to frontend
                let _ = app_handle.emit("item-updated", &item.id);
            }
        }

        Ok(())
    }

    fn is_permanent_github_error(item_type: &str, error: &str) -> bool {
        if item_type != "github_action" && item_type != "github_pr" {
            return false;
        }

        error.contains("GitHub API error: 401")
            || error.contains("GitHub API error: 403")
            || error.contains("GitHub API error: 404")
    }

    async fn poll_slack_thread(db: &Arc<Database>, item: &crate::db::Item) -> anyhow::Result<()> {
        let token = db.get_credential("slack_token")?
            .ok_or_else(|| anyhow::anyhow!("Slack token not configured"))?;

        let metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let channel_id = metadata["channel_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing channel_id"))?;
        let thread_ts = metadata["thread_ts"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing thread_ts"))?;

        let result = slack::check_slack_thread(&token, channel_id, thread_ts).await?;

        // Check if message count changed
        let old_metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let old_count = old_metadata["message_count"].as_i64().unwrap_or(0);
        let new_count = result["message_count"].as_i64().unwrap_or(0);

        if new_count > old_count {
            // Update status to "updated"
            let mut result_with_identifiers = result;
            result_with_identifiers.insert("channel_id".to_string(), serde_json::json!(channel_id));
            result_with_identifiers.insert("thread_ts".to_string(), serde_json::json!(thread_ts));
            let new_metadata = serde_json::to_string(&result_with_identifiers)?;
            db.update_item_status(&item.id, "updated", Some(&new_metadata))?;
        } else {
            // Just update last_checked_at
            db.update_item_status(&item.id, &item.status, None)?;
        }

        Ok(())
    }

    async fn poll_github_action(
        db: &Arc<Database>,
        item: &crate::db::Item,
    ) -> anyhow::Result<()> {
        let token = db
            .get_credential("github_token")?
            .unwrap_or_default();

        let metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let owner = Self::resolve_metadata_field(item, &metadata, "owner")?;
        let repo = Self::resolve_metadata_field(item, &metadata, "repo")?;
        let run_id = Self::resolve_metadata_field(item, &metadata, "run_id")?;

        let result = github_actions::check_github_action(&token, &owner, &repo, &run_id).await?;

        // Determine new status based on GitHub Action status
        let status = result["status"].as_str().unwrap_or("unknown");
        let conclusion = result["conclusion"].as_str();

        let new_status = match status {
            "queued" | "waiting" => "waiting",
            "in_progress" => "in_progress",
            "completed" => {
                match conclusion {
                    Some("success") => "completed",
                    Some("failure") | Some("cancelled") => "failed",
                    _ => "completed",
                }
            }
            _ => "waiting",
        };

        // Update if status changed
        let metadata_missing_ids = metadata["owner"].as_str().is_none()
            || metadata["repo"].as_str().is_none()
            || metadata["run_id"].as_str().is_none();

        if new_status != item.status || metadata_missing_ids {
            let mut result_with_identifiers = result;
            result_with_identifiers.insert("owner".to_string(), serde_json::json!(owner));
            result_with_identifiers.insert("repo".to_string(), serde_json::json!(repo));
            result_with_identifiers.insert("run_id".to_string(), serde_json::json!(run_id));
            let new_metadata = serde_json::to_string(&result_with_identifiers)?;
            db.update_item_status(&item.id, new_status, Some(&new_metadata))?;
        } else {
            db.update_item_status(&item.id, &item.status, None)?;
        }

        Ok(())
    }

    async fn poll_github_pr(db: &Arc<Database>, item: &crate::db::Item) -> anyhow::Result<()> {
        let token = db
            .get_credential("github_token")?
            .unwrap_or_default();

        let metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let owner = Self::resolve_metadata_field(item, &metadata, "owner")?;
        let repo = Self::resolve_metadata_field(item, &metadata, "repo")?;
        let pr_number = Self::resolve_metadata_field(item, &metadata, "pr_number")?;

        let result = github_pr::check_github_pr(&token, &owner, &repo, &pr_number).await?;

        // Check for changes
        let old_metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let old_review_count = old_metadata["review_count"].as_i64().unwrap_or(0);
        let new_review_count = result["review_count"].as_i64().unwrap_or(0);

        let state = result["state"].as_str().unwrap_or("open");
        let merged = result["merged"].as_bool().unwrap_or(false);
        let has_approval = result["has_approval"].as_bool().unwrap_or(false);
        let has_changes_requested = result["has_changes_requested"].as_bool().unwrap_or(false);
        let metadata_missing_ids = metadata["owner"].as_str().is_none()
            || metadata["repo"].as_str().is_none()
            || metadata["pr_number"].as_str().is_none();

        let new_status = if merged || state == "closed" {
            "completed"
        } else if new_review_count > old_review_count || has_approval || has_changes_requested {
            "updated"
        } else {
            "in_progress"
        };

        if new_status != item.status || new_review_count > old_review_count || metadata_missing_ids {
            let mut result_with_identifiers = result;
            result_with_identifiers.insert("owner".to_string(), serde_json::json!(owner));
            result_with_identifiers.insert("repo".to_string(), serde_json::json!(repo));
            result_with_identifiers.insert("pr_number".to_string(), serde_json::json!(pr_number));
            let new_metadata = serde_json::to_string(&result_with_identifiers)?;
            db.update_item_status(&item.id, new_status, Some(&new_metadata))?;
        } else {
            db.update_item_status(&item.id, &item.status, None)?;
        }

        Ok(())
    }

    fn resolve_metadata_field(
        item: &crate::db::Item,
        metadata: &serde_json::Value,
        key: &str,
    ) -> anyhow::Result<String> {
        if let Some(value) = metadata[key].as_str() {
            return Ok(value.to_string());
        }

        if let Some(url) = &item.url {
            if let Ok(parsed) = url_parser::parse_url(url) {
                if let Some(value) = parsed.metadata.get(key) {
                    return Ok(value.clone());
                }
            }
        }

        Err(anyhow::anyhow!("Missing {}", key))
    }

    async fn get_opencode_context(
        db: &Arc<Database>,
    ) -> Option<(String, String, HashMap<String, opencode::SessionStatus>)> {
        let raw_url = db.get_credential("opencode_url").ok().flatten()?;
        if raw_url.is_empty() {
            return None;
        }
        let config = opencode::parse_opencode_url(&raw_url).ok()?;
        let password = db
            .get_credential("opencode_password")
            .ok()
            .flatten()
            .unwrap_or_default();

        // Fetch statuses from ALL directories so sessions across projects get correct status
        let directories = opencode::enumerate_opencode_directories();
        let mut all_statuses = HashMap::new();
        for dir in &directories {
            if let Ok(statuses) = opencode::get_session_statuses(&config.base_url, &password, Some(dir)).await {
                all_statuses.extend(statuses);
            }
        }

        Some((config.base_url, password, all_statuses))
    }

    async fn discover_opencode_sessions(
        db: &Arc<Database>,
        app_handle: &AppHandle,
    ) -> anyhow::Result<()> {
        let raw_url = match db.get_credential("opencode_url")? {
            Some(u) if !u.is_empty() => u,
            _ => return Ok(()),
        };
        let config = opencode::parse_opencode_url(&raw_url)?;
        let password = db
            .get_credential("opencode_password")?
            .unwrap_or_default();

        if !opencode::check_opencode_health(&config.base_url, &password).await? {
            return Ok(());
        }

        let directories = opencode::enumerate_opencode_directories();
        let existing_session_ids = db.get_opencode_session_ids()?;

        for dir in &directories {
            let sessions = match opencode::list_sessions(&config.base_url, &password, Some(dir)).await {
                Ok(s) => s,
                Err(_) => continue,
            };
            let statuses = opencode::get_session_statuses(&config.base_url, &password, Some(dir))
                .await
                .unwrap_or_default();
            let web_url = opencode::build_web_url(&config.base_url, dir);

            for session in &sessions {
                if existing_session_ids.contains(&session.id) {
                    continue;
                }

                if session.parent_id.is_some() {
                    continue;
                }

                let status_str = if session.time.archived.is_some() {
                    "archived"
                } else {
                    match statuses.get(&session.id) {
                        Some(opencode::SessionStatus::Busy) => "in_progress",
                        Some(opencode::SessionStatus::Retry { .. }) => "in_progress",
                        Some(opencode::SessionStatus::Idle) | None => "completed",
                    }
                };

                let title = if session.title.is_empty() {
                    format!("OpenCode Session {}", &session.id[..8.min(session.id.len())])
                } else {
                    session.title.clone()
                };

                let metadata = serde_json::json!({
                    "session_id": session.id,
                    "opencode_url": web_url,
                    "directory": dir,
                    "session_status": match statuses.get(&session.id) {
                        Some(opencode::SessionStatus::Idle) => "idle",
                        Some(opencode::SessionStatus::Busy) => "busy",
                        Some(opencode::SessionStatus::Retry { .. }) => "retry",
                        None => "unknown",
                    },
                    "session_title": session.title,
                    "last_activity": session.time.updated,
                });

                let item = Item {
                    id: uuid::Uuid::new_v4().to_string(),
                    item_type: "opencode_session".to_string(),
                    title,
                    url: None,
                    status: status_str.to_string(),
                    previous_status: None,
                    metadata: serde_json::to_string(&metadata)?,
                    last_checked_at: None,
                    last_updated_at: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    archived: false,
                    polling_interval_override: None,
                    checked: status_str == "archived",
                };

                db.add_item(&item)?;
                let _ = app_handle.emit("item-updated", &item.id);
            }
        }

        Ok(())
    }

    async fn poll_opencode_session(
        db: &Arc<Database>,
        item: &crate::db::Item,
        context: &Option<(String, String, HashMap<String, opencode::SessionStatus>)>,
        app_handle: &AppHandle,
    ) -> anyhow::Result<()> {
        let (url, password, statuses) = match context {
            Some(ctx) => (&ctx.0, &ctx.1, &ctx.2),
            None => {
                db.update_item_status(&item.id, &item.status, None)?;
                return Ok(());
            }
        };

        let metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let session_id = metadata["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing session_id in opencode_session metadata"))?;
        let stored_dir = metadata["directory"].as_str().map(|s| s.to_string());
        let resolved_dir = stored_dir.or_else(|| opencode::find_session_directory(session_id));

        let result =
            opencode::poll_opencode_session(url, password, session_id, statuses).await?;

        let session_status = result
            .get("session_status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let sessions = opencode::list_sessions(url, password, resolved_dir.as_deref()).await?;
        let is_archived = sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.time.archived.is_some())
            .unwrap_or(false);

        let new_status = if is_archived {
            "archived"
        } else {
            match session_status {
                "busy" | "retry" => "in_progress",
                _ => "completed",
            }
        };

        let mut full_metadata = result;
        if let Some(ref dir) = resolved_dir {
            let web_url = opencode::build_web_url(url, dir);
            full_metadata.insert("opencode_url".to_string(), serde_json::json!(web_url));
            full_metadata.insert("directory".to_string(), serde_json::json!(dir));
        } else {
            full_metadata.insert("opencode_url".to_string(), serde_json::json!(url));
        }
        if let Some(title) = sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| &s.title)
        {
            full_metadata.insert("session_title".to_string(), serde_json::json!(title));
        }
        if let Some(activity) = sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.time.updated)
        {
            full_metadata.insert("last_activity".to_string(), serde_json::json!(activity));
        }

        let new_metadata = serde_json::to_string(&full_metadata)?;
        db.update_item_status(&item.id, new_status, Some(&new_metadata))?;

        if new_status != item.status {
            let notification_body = match (item.status.as_str(), new_status) {
                ("in_progress", "completed") => Some("Waiting for your input"),
                (_, "archived") => Some("Session has been archived"),
                ("completed", "in_progress") => Some("Agent started working"),
                _ => None,
            };

            if let Some(body) = notification_body {
                let _ = app_handle
                    .notification()
                    .builder()
                    .title(&item.title)
                    .body(body)
                    .show();
            }
        }

        if new_status == "archived" && item.status != "archived" {
            db.toggle_checked(&item.id, true)?;
        }

        // Auto-uncheck when session becomes active again (completed/failed → waiting/in_progress)
        if (item.status == "completed" || item.status == "failed")
            && (new_status == "waiting" || new_status == "in_progress")
        {
            db.toggle_checked(&item.id, false)?;
        }

        Ok(())
    }
}
