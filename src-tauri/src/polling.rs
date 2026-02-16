use crate::db::Database;
use crate::services::{github_actions, github_pr, slack};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
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
        let items = db.get_items(false)?;

        for item in items {
            // Skip if item is completed or failed
            if item.status == "completed" || item.status == "failed" {
                continue;
            }

            let result = match item.item_type.as_str() {
                "slack_thread" => Self::poll_slack_thread(db, &item).await,
                "github_action" => Self::poll_github_action(db, &item).await,
                "github_pr" => Self::poll_github_pr(db, &item).await,
                _ => continue,
            };

            if let Err(e) = result {
                eprintln!("Error polling item {}: {}", item.id, e);
            } else {
                // Emit event to frontend
                let _ = app_handle.emit("item-updated", &item.id);
            }
        }

        Ok(())
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
            let new_metadata = serde_json::to_string(&result)?;
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
        let token = db.get_credential("github_token")?
            .ok_or_else(|| anyhow::anyhow!("GitHub token not configured"))?;

        let metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let owner = metadata["owner"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
        let repo = metadata["repo"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing repo"))?;
        let run_id = metadata["run_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing run_id"))?;

        let result = github_actions::check_github_action(&token, owner, repo, run_id).await?;

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
        if new_status != item.status {
            let new_metadata = serde_json::to_string(&result)?;
            db.update_item_status(&item.id, new_status, Some(&new_metadata))?;
        } else {
            db.update_item_status(&item.id, &item.status, None)?;
        }

        Ok(())
    }

    async fn poll_github_pr(db: &Arc<Database>, item: &crate::db::Item) -> anyhow::Result<()> {
        let token = db.get_credential("github_token")?
            .ok_or_else(|| anyhow::anyhow!("GitHub token not configured"))?;

        let metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let owner = metadata["owner"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
        let repo = metadata["repo"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing repo"))?;
        let pr_number = metadata["pr_number"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing pr_number"))?;

        let result = github_pr::check_github_pr(&token, owner, repo, pr_number).await?;

        // Check for changes
        let old_metadata: serde_json::Value = serde_json::from_str(&item.metadata)?;
        let old_review_count = old_metadata["review_count"].as_i64().unwrap_or(0);
        let new_review_count = result["review_count"].as_i64().unwrap_or(0);

        let state = result["state"].as_str().unwrap_or("open");
        let merged = result["merged"].as_bool().unwrap_or(false);

        let new_status = if merged || state == "closed" {
            "completed"
        } else if new_review_count > old_review_count {
            "updated"
        } else {
            &item.status
        };

        if new_status != item.status || new_review_count > old_review_count {
            let new_metadata = serde_json::to_string(&result)?;
            db.update_item_status(&item.id, new_status, Some(&new_metadata))?;
        } else {
            db.update_item_status(&item.id, &item.status, None)?;
        }

        Ok(())
    }
}
