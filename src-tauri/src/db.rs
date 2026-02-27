use anyhow::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub title: String,
    pub url: Option<String>,
    pub status: String,
    pub previous_status: Option<String>,
    pub metadata: String, // JSON blob
    pub last_checked_at: Option<String>,
    pub last_updated_at: Option<String>,
    pub created_at: String,
    pub archived: bool,
    pub archived_at: Option<String>,
    pub polling_interval_override: Option<i64>,
    pub checked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub slack_token: Option<String>,
    pub github_token: Option<String>,
    pub opencode_url: Option<String>,
    pub opencode_password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub polling_interval: i64,
    pub screen_width: i64,
}

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Create tables
        conn.execute(
            "CREATE TABLE IF NOT EXISTS items (
                id TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                title TEXT NOT NULL,
                url TEXT,
                status TEXT NOT NULL DEFAULT 'waiting',
                previous_status TEXT,
                metadata TEXT NOT NULL,
                last_checked_at TEXT,
                last_updated_at TEXT,
                created_at TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0,
                polling_interval_override INTEGER,
                checked INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        // Migration: add checked column if missing
        let has_checked = conn.prepare("SELECT checked FROM items LIMIT 0").is_ok();
        if !has_checked {
            conn.execute(
                "ALTER TABLE items ADD COLUMN checked INTEGER NOT NULL DEFAULT 0",
                [],
            )?;
        }

        // Migration: add archived_at column if missing
        let has_archived_at = conn.prepare("SELECT archived_at FROM items LIMIT 0").is_ok();
        if !has_archived_at {
            conn.execute(
                "ALTER TABLE items ADD COLUMN archived_at TEXT",
                [],
            )?;
        }

        conn.execute(
            "CREATE TABLE IF NOT EXISTS credentials (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        // Set default polling interval if not exists
        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('polling_interval', '30')",
            [],
        )?;

        conn.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES ('screen_width', '400')",
            [],
        )?;

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn add_item(&self, item: &Item) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO items (id, type, title, url, status, previous_status, metadata, 
                               last_checked_at, last_updated_at, created_at, archived, polling_interval_override, checked, archived_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                item.id,
                item.item_type,
                item.title,
                item.url,
                item.status,
                item.previous_status,
                item.metadata,
                item.last_checked_at,
                item.last_updated_at,
                item.created_at,
                item.archived as i32,
                item.polling_interval_override,
                item.checked as i32,
                item.archived_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_items(&self, archived: bool) -> Result<Vec<Item>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, title, url, status, previous_status, metadata,
                    last_checked_at, last_updated_at, created_at, archived, polling_interval_override, checked, archived_at
             FROM items WHERE archived = ?1 ORDER BY created_at DESC"
        )?;

        let items = stmt
            .query_map([archived as i32], |row| {
                Ok(Item {
                    id: row.get(0)?,
                    item_type: row.get(1)?,
                    title: row.get(2)?,
                    url: row.get(3)?,
                    status: row.get(4)?,
                    previous_status: row.get(5)?,
                    metadata: row.get(6)?,
                    last_checked_at: row.get(7)?,
                    last_updated_at: row.get(8)?,
                    created_at: row.get(9)?,
                    archived: row.get::<_, i32>(10)? != 0,
                    polling_interval_override: row.get(11)?,
                    checked: row.get::<_, i32>(12)? != 0,
                    archived_at: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub fn update_item_status(&self, id: &str, status: &str, metadata: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // First, get the current status to save as previous_status
        let mut stmt = conn.prepare("SELECT status FROM items WHERE id = ?1")?;
        let current_status: String = stmt.query_row([id], |row| row.get(0))?;

        if let Some(meta) = metadata {
            conn.execute(
                "UPDATE items SET status = ?1, previous_status = ?2, 
                 last_checked_at = ?3, last_updated_at = ?3, metadata = ?4
                 WHERE id = ?5",
                params![status, current_status, now, meta, id],
            )?;
        } else {
            conn.execute(
                "UPDATE items SET status = ?1, previous_status = ?2,
                 last_checked_at = ?3, last_updated_at = ?3
                 WHERE id = ?4",
                params![status, current_status, now, id],
            )?;
        }
        Ok(())
    }

    pub fn touch_item_check(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE items SET last_checked_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn update_item_title(&self, id: &str, title: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET title = ?1 WHERE id = ?2",
            params![title, id],
        )?;
        Ok(())
    }

    pub fn update_item_poll_error(&self, id: &str, error: &str, mark_failed: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        let mut stmt = conn.prepare("SELECT status, metadata FROM items WHERE id = ?1")?;
        let (current_status, metadata_str): (String, String) =
            stmt.query_row([id], |row| Ok((row.get(0)?, row.get(1)?)))?;

        let mut metadata_value = serde_json::from_str::<serde_json::Value>(&metadata_str)
            .unwrap_or_else(|_| serde_json::json!({}));

        if !metadata_value.is_object() {
            metadata_value = serde_json::json!({});
        }

        if let Some(map) = metadata_value.as_object_mut() {
            map.insert("last_error".to_string(), serde_json::json!(error));
            map.insert("last_error_at".to_string(), serde_json::json!(now));
        }

        let new_metadata = serde_json::to_string(&metadata_value)?;

        if mark_failed && current_status != "failed" {
            conn.execute(
                "UPDATE items SET status = 'failed', previous_status = ?1,
                 last_checked_at = ?2, last_updated_at = ?2, metadata = ?3
                 WHERE id = ?4",
                params![current_status, now, new_metadata, id],
            )?;
        } else {
            conn.execute(
                "UPDATE items SET last_checked_at = ?1, metadata = ?2 WHERE id = ?3",
                params![now, new_metadata, id],
            )?;
        }

        Ok(())
    }

    pub fn remove_item(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM items WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn toggle_checked(&self, id: &str, checked: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET checked = ?1 WHERE id = ?2",
            params![checked as i32, id],
        )?;
        Ok(())
    }

    pub fn archive_item(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE items SET archived = 1, archived_at = ?1, checked = 0 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn archive_items(&self, ids: &[String]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        for id in ids {
            conn.execute(
                "UPDATE items SET archived = 1, archived_at = ?1, checked = 0 WHERE id = ?2",
                params![now, id],
            )?;
        }
        Ok(())
    }

    pub fn unarchive_item(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE items SET archived = 0, archived_at = NULL WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn cleanup_old_archived(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
        let count = conn.execute(
            "DELETE FROM items WHERE archived = 1 AND archived_at IS NOT NULL AND archived_at < ?1",
            params![cutoff],
        )?;
        Ok(count as u64)
    }

    pub fn archive_closed_items(&self) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let count = conn.execute(
            "UPDATE items SET archived = 1, archived_at = ?1, checked = 0
             WHERE archived = 0
               AND status = 'closed'",
            params![now],
        )?;
        Ok(count as u64)
    }

    pub fn archive_stale_items(&self, before: &str) -> Result<u64> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let count = conn.execute(
            "UPDATE items SET archived = 1, archived_at = ?1, checked = 0
             WHERE archived = 0
               AND COALESCE(last_updated_at, created_at) < ?2",
            params![now, before],
        )?;
        Ok(count as u64)
    }

    pub fn get_opencode_session_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT metadata FROM items WHERE type = 'opencode_session'")?;
        let ids = stmt
            .query_map([], |row| {
                let meta: String = row.get(0)?;
                Ok(meta)
            })?
            .filter_map(|m| {
                m.ok().and_then(|meta_str| {
                    serde_json::from_str::<serde_json::Value>(&meta_str)
                        .ok()
                        .and_then(|v| v["session_id"].as_str().map(|s| s.to_string()))
                })
            })
            .collect();
        Ok(ids)
    }

    /// Remove any copilot_agent items that track the given copilot session id.
    /// Used when a cli_session claims the same session to avoid duplicates.
    pub fn remove_copilot_agent_by_session_id(&self, copilot_session_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        // Find matching copilot_agent item ids
        let mut stmt = conn.prepare(
            "SELECT id, metadata FROM items WHERE type = 'copilot_agent'",
        )?;
        let ids_to_remove: Vec<String> = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let meta: String = row.get(1)?;
                Ok((id, meta))
            })?
            .filter_map(|r| {
                r.ok().and_then(|(id, meta_str)| {
                    serde_json::from_str::<serde_json::Value>(&meta_str)
                        .ok()
                        .and_then(|v| {
                            if v["copilot_session_id"].as_str() == Some(copilot_session_id) {
                                Some(id)
                            } else {
                                None
                            }
                        })
                })
            })
            .collect();

        for id in &ids_to_remove {
            conn.execute("DELETE FROM items WHERE id = ?1", params![id])?;
        }
        Ok(ids_to_remove)
    }

    pub fn get_copilot_session_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT metadata FROM items WHERE type IN ('copilot_agent', 'cli_session')",
        )?;
        let ids = stmt
            .query_map([], |row| {
                let meta: String = row.get(0)?;
                Ok(meta)
            })?
            .filter_map(|m| {
                m.ok().and_then(|meta_str| {
                    serde_json::from_str::<serde_json::Value>(&meta_str)
                        .ok()
                        .and_then(|v| v["copilot_session_id"].as_str().map(|s| s.to_string()))
                })
            })
            .collect();
        Ok(ids)
    }

    /// Close active copilot sessions at the given CWD, excluding a specific session ID.
    /// Returns the IDs of items that were closed.
    pub fn close_copilot_sessions_at_cwd(
        &self,
        cwd: &str,
        exclude_session_id: &str,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, metadata FROM items
             WHERE type IN ('copilot_agent', 'cli_session')
               AND status NOT IN ('closed', 'archived')",
        )?;
        let candidates: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();

        let mut closed_ids = Vec::new();
        for (item_id, meta_str) in candidates {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                let item_cwd = meta["cwd"].as_str().unwrap_or("");
                let item_sid = meta["copilot_session_id"].as_str().unwrap_or("");
                if item_cwd == cwd && item_sid != exclude_session_id {
                    conn.execute(
                        "UPDATE items SET previous_status = status, status = 'closed', last_updated_at = datetime('now') WHERE id = ?1",
                        params![item_id],
                    )?;
                    closed_ids.push(item_id);
                }
            }
        }
        Ok(closed_ids)
    }

    pub fn save_credential(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO credentials (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_credential(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM credentials WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_visible_items(&self) -> Result<Vec<Item>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, title, url, status, previous_status, metadata,
                    last_checked_at, last_updated_at, created_at, archived, polling_interval_override, checked, archived_at
             FROM items WHERE archived = 0 AND checked = 0 ORDER BY created_at DESC"
        )?;

        let items = stmt
            .query_map([], |row| {
                Ok(Item {
                    id: row.get(0)?,
                    item_type: row.get(1)?,
                    title: row.get(2)?,
                    url: row.get(3)?,
                    status: row.get(4)?,
                    previous_status: row.get(5)?,
                    metadata: row.get(6)?,
                    last_checked_at: row.get(7)?,
                    last_updated_at: row.get(8)?,
                    created_at: row.get(9)?,
                    archived: row.get::<_, i32>(10)? != 0,
                    polling_interval_override: row.get(11)?,
                    checked: row.get::<_, i32>(12)? != 0,
                    archived_at: row.get(13)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(items)
    }

    pub fn count_actionable_items(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM items
             WHERE archived = 0
               AND checked = 0
                AND status IN ('completed', 'failed', 'updated', 'approved', 'merged', 'waiting', 'input_needed')",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_all_settings(&self) -> Result<Settings> {
        let polling_interval = self
            .get_setting("polling_interval")?
            .unwrap_or_else(|| "30".to_string())
            .parse()
            .unwrap_or(30);

        let screen_width = self
            .get_setting("screen_width")?
            .unwrap_or_else(|| "400".to_string())
            .parse()
            .unwrap_or(400);

        Ok(Settings {
            polling_interval,
            screen_width,
        })
    }
}
