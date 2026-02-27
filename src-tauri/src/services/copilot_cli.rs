use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::Command;

const MAX_TITLE_LEN: usize = 80;

/// Truncate a title to MAX_TITLE_LEN chars, appending "…" if trimmed.
pub fn truncate_title(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= MAX_TITLE_LEN {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(MAX_TITLE_LEN - 1).collect();
        format!("{}…", truncated)
    }
}

/// Detected runtime status of a Copilot CLI session.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionActivity {
    /// Agent is actively working (tool calls, generating responses).
    InProgress,
    /// Agent finished its turn and is waiting for user input.
    InputNeeded,
    /// Session appears idle / no recent activity.
    Idle,
}

#[derive(Debug, Clone)]
pub struct CopilotSession {
    pub id: String,
    pub name: Option<String>,
    pub summary: Option<String>,
    pub cwd: Option<String>,
    pub repository: Option<String>,
    pub branch: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl CopilotSession {
    /// Returns the best display name: explicit name > summary > None
    pub fn display_name(&self) -> Option<&str> {
        self.name
            .as_deref()
            .or(self.summary.as_deref())
    }
}

fn session_state_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".copilot").join("session-state"))
}

fn parse_workspace_yaml(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in content.lines() {
        if let Some((key, value)) = line.split_once(": ") {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

pub fn discover_sessions() -> Vec<CopilotSession> {
    let base = match session_state_dir() {
        Some(p) if p.is_dir() => p,
        _ => return vec![],
    };

    let mut sessions = Vec::new();

    let entries = match fs::read_dir(&base) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let workspace_file = path.join("workspace.yaml");
        if !workspace_file.exists() {
            continue;
        }

        let content = match fs::read_to_string(&workspace_file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let fields = parse_workspace_yaml(&content);

        let id = match fields.get("id") {
            Some(id) => id.clone(),
            None => continue,
        };

        sessions.push(CopilotSession {
            id,
            name: fields.get("name").cloned().filter(|s| !s.is_empty()),
            summary: fields.get("summary").cloned().filter(|s| !s.is_empty()),
            cwd: fields.get("cwd").cloned(),
            repository: fields.get("repository").cloned(),
            branch: fields.get("branch").cloned(),
            created_at: fields.get("created_at").cloned(),
            updated_at: fields.get("updated_at").cloned(),
        });
    }

    sessions
}

pub fn read_session(session_id: &str) -> Option<CopilotSession> {
    let base = session_state_dir()?;
    let workspace_file = base.join(session_id).join("workspace.yaml");
    let content = fs::read_to_string(&workspace_file).ok()?;
    let fields = parse_workspace_yaml(&content);
    let id = fields.get("id")?.clone();

    Some(CopilotSession {
        id,
        name: fields.get("name").cloned().filter(|s| !s.is_empty()),
        summary: fields.get("summary").cloned().filter(|s| !s.is_empty()),
        cwd: fields.get("cwd").cloned(),
        repository: fields.get("repository").cloned(),
        branch: fields.get("branch").cloned(),
        created_at: fields.get("created_at").cloned(),
        updated_at: fields.get("updated_at").cloned(),
    })
}

/// Find a Copilot CLI session whose created_at is closest to the given
/// timestamp (within a 2-minute window).
pub fn find_session_by_time(created_at: &str) -> Option<CopilotSession> {
    let target = chrono::DateTime::parse_from_rfc3339(created_at).ok()?;
    let sessions = discover_sessions();
    let max_delta = chrono::Duration::seconds(15);

    sessions
        .into_iter()
        .filter_map(|s| {
            let t = chrono::DateTime::parse_from_rfc3339(s.created_at.as_deref()?).ok()?;
            let delta = (t - target).abs();
            if delta <= max_delta {
                Some((s, delta))
            } else {
                None
            }
        })
        .min_by_key(|(_, delta)| *delta)
        .map(|(s, _)| s)
}

/// Find the most recently updated Copilot CLI session matching the given cwd.
pub fn find_session_by_cwd(cwd: &str) -> Option<CopilotSession> {
    let sessions = discover_sessions();

    sessions
        .into_iter()
        .filter(|s| s.cwd.as_deref() == Some(cwd))
        .max_by(|a, b| {
            let ta = a.updated_at.as_deref().unwrap_or("");
            let tb = b.updated_at.as_deref().unwrap_or("");
            ta.cmp(tb)
        })
}

/// Determine the live activity status of a session by reading events.jsonl.
///
/// Strategy: read the tail of events.jsonl and check event types.
/// - `task_complete` tool in recent events → Idle (completed)
/// - `ask_user` tool pending → InputNeeded
/// - `assistant.turn_end` → InputNeeded (or Idle if >2 min old)
/// - Other events → InProgress (or Idle if >2 min old)
pub fn detect_session_activity(session_id: &str) -> SessionActivity {
    let base = match session_state_dir() {
        Some(p) => p,
        None => return SessionActivity::Idle,
    };

    let events_file = base.join(session_id).join("events.jsonl");
    let recent_events = match read_tail_events(&events_file, 10) {
        Some(e) if !e.is_empty() => e,
        _ => return SessionActivity::Idle,
    };

    // Check if task_complete was called in recent events → session is done
    let has_task_complete = recent_events.iter().any(|e| {
        e.get("type").and_then(|v| v.as_str()) == Some("tool.execution_start")
            && e.get("data")
                .and_then(|d| d.get("toolName"))
                .and_then(|v| v.as_str())
                == Some("task_complete")
    });
    if has_task_complete {
        return SessionActivity::Idle;
    }

    let last_event = &recent_events[recent_events.len() - 1];

    let event_type = last_event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Check if a tool execution is actually waiting for user input (e.g. ask_user)
    let is_user_input_tool = event_type == "tool.execution_start"
        && last_event
            .get("data")
            .and_then(|d| d.get("toolName"))
            .and_then(|v| v.as_str())
            .map(|name| name == "ask_user" || name == "askUser")
            .unwrap_or(false);

    let is_stale = last_event
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|ts| {
            let age = chrono::Utc::now().signed_duration_since(ts);
            age > chrono::Duration::minutes(2)
        })
        .unwrap_or(true);

    match event_type {
        // tool.execution_start for ask_user means waiting for user input
        "tool.execution_start" if is_user_input_tool => {
            if is_stale {
                SessionActivity::Idle
            } else {
                SessionActivity::InputNeeded
            }
        }

        // Agent is actively generating/working
        "assistant.turn_start" | "assistant.message" | "tool.execution_start"
        | "tool.execution_complete" | "subagent.started" | "subagent.completed"
        | "session.mode_changed" | "session.context_changed" => {
            if is_stale {
                SessionActivity::Idle
            } else {
                SessionActivity::InProgress
            }
        }

        // Agent finished a turn — waiting for user (or idle if stale)
        "assistant.turn_end" => {
            if is_stale {
                SessionActivity::Idle
            } else {
                SessionActivity::InputNeeded
            }
        }

        // User just sent a message — agent will start soon
        "user.message" => {
            if is_stale {
                SessionActivity::Idle
            } else {
                SessionActivity::InProgress
            }
        }

        // Session lifecycle events
        "session.start" | "session.info" | "session.model_change" => {
            if is_stale {
                SessionActivity::Idle
            } else {
                SessionActivity::InProgress
            }
        }

        // Session error — treat as idle
        "session.error" => SessionActivity::Idle,

        _ => SessionActivity::Idle,
    }
}

/// Extract the first user message content from events.jsonl.
/// Useful for auto-generating a session title.
pub fn first_user_message(session_id: &str) -> Option<String> {
    let base = session_state_dir()?;
    let events_file = base.join(session_id).join("events.jsonl");
    let file = fs::File::open(&events_file).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().take(50) {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let obj: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if obj.get("type").and_then(|v| v.as_str()) == Some("user.message") {
            let content = obj
                .get("data")
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
                .unwrap_or("");
            if !content.is_empty() {
                return Some(truncate_title(content));
            }
        }
    }
    None
}

/// Get the timestamp of the last event in events.jsonl for a session.
pub fn last_event_timestamp(session_id: &str) -> Option<String> {
    let base = session_state_dir()?;
    let events_file = base.join(session_id).join("events.jsonl");
    let events = read_tail_events(&events_file, 1)?;
    events
        .last()
        .and_then(|e| e.get("timestamp"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Get the set of working directories where a `copilot` process is currently running.
/// Uses `lsof` on macOS to inspect the cwd of copilot processes.
pub fn get_active_copilot_cwds() -> HashSet<String> {
    let output = Command::new("lsof")
        .args(["-a", "-d", "cwd", "-c", "copilot", "-Fn"])
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return HashSet::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .map(|s| s.to_string())
        .collect()
}

/// Check whether a Copilot CLI session's process is still running.
/// Compares the session's cwd against the set of active copilot process cwds.
pub fn is_session_process_running(session: &CopilotSession, active_cwds: &HashSet<String>) -> bool {
    match &session.cwd {
        Some(cwd) => active_cwds.contains(cwd),
        None => false,
    }
}

/// Read the last N valid JSON events from an events.jsonl file.
/// Uses a tail-read approach for efficiency.
fn read_tail_events(path: &PathBuf, count: usize) -> Option<Vec<serde_json::Value>> {
    let mut file = fs::File::open(path).ok()?;
    let file_len = file.metadata().ok()?.len();

    if file_len == 0 {
        return None;
    }

    // Read the last 16KB — enough for many events
    let read_size = std::cmp::min(file_len, 16384);
    let start = file_len - read_size;
    file.seek(SeekFrom::Start(start)).ok()?;

    let mut buf = String::new();
    file.read_to_string(&mut buf).ok()?;

    let mut events = Vec::new();
    for line in buf.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            events.push(val);
            if events.len() >= count {
                break;
            }
        }
    }

    events.reverse();
    if events.is_empty() {
        None
    } else {
        Some(events)
    }
}
