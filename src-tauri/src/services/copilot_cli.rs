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
/// - `assistant.turn_end` → Idle (agent finished its turn)
/// - Other events → InProgress (or Idle if >2 min old)
pub fn detect_session_activity(session_id: &str, process_running: bool) -> SessionActivity {
    let base = match session_state_dir() {
        Some(p) => p,
        None => return SessionActivity::Idle,
    };

    let events_file = base.join(session_id).join("events.jsonl");
    let recent_events = match read_tail_events(&events_file, 30) {
        Some(e) if !e.is_empty() => e,
        _ => {
            // If we can't read events but the process is still running,
            // assume the agent is active (e.g. during compaction the file
            // may be temporarily empty/rewritten).
            if process_running {
                return SessionActivity::InProgress;
            }
            return SessionActivity::Idle;
        }
    };

    classify_events(&recent_events, process_running)
}

/// Threshold (seconds) after which an active turn with no new events
/// is considered to be waiting for user confirmation.
/// Model thinking is typically <30s; 60s strongly suggests a confirmation prompt.
const TOOL_CONFIRMATION_THRESHOLD_SECS: i64 = 60;

/// Threshold (seconds) for workspace trust prompt detection.
/// If session started but no user.message exists after this time, likely a trust prompt.
const WORKSPACE_TRUST_THRESHOLD_SECS: i64 = 15;

/// Core classification logic, extracted for testability.
fn classify_events(recent_events: &[serde_json::Value], process_running: bool) -> SessionActivity {
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

    let last_event_age_secs = last_event
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
        .map(|ts| chrono::Utc::now().signed_duration_since(ts).num_seconds())
        .unwrap_or(i64::MAX);

    let is_stale = last_event_age_secs > 120; // 2 minutes

    // Check if a tool execution is actually waiting for user input (e.g. ask_user)
    let is_user_input_tool = event_type == "tool.execution_start"
        && last_event
            .get("data")
            .and_then(|d| d.get("toolName"))
            .and_then(|v| v.as_str())
            .map(|name| name == "ask_user" || name == "askUser")
            .unwrap_or(false);

    if is_user_input_tool {
        return if process_running || !is_stale {
            SessionActivity::InputNeeded
        } else {
            SessionActivity::Idle
        };
    }

    // Check if we're in an active turn (turn_start without matching turn_end)
    let in_active_turn = {
        let last_turn_start = recent_events
            .iter()
            .rposition(|e| e.get("type").and_then(|v| v.as_str()) == Some("assistant.turn_start"));
        let last_turn_end = recent_events
            .iter()
            .rposition(|e| e.get("type").and_then(|v| v.as_str()) == Some("assistant.turn_end"));
        match (last_turn_start, last_turn_end) {
            (Some(start), Some(end)) => start > end,
            (Some(_), None) => true,
            _ => false,
        }
    };

    // Heuristic: CLI tool confirmation prompt ("Do you want to run this command?")
    //
    // The CLI buffers events — assistant.message and tool.execution_start are written
    // together AFTER the user confirms. During the confirmation prompt, events.jsonl
    // still shows the previous step's events (e.g. tool.execution_complete or
    // assistant.turn_start). We detect this by checking if significant time has passed
    // since the last event while we're in an active turn and no tool is actively running.
    let is_actively_executing = matches!(
        event_type,
        "tool.execution_start" | "subagent.started" | "session.compaction_start"
    );
    if process_running
        && in_active_turn
        && !is_actively_executing
        && last_event_age_secs > TOOL_CONFIRMATION_THRESHOLD_SECS
    {
        return SessionActivity::InputNeeded;
    }

    // Heuristic: workspace trust prompt ("Do you want to add these directories?")
    //
    // This prompt appears BEFORE any agent activity — no user.message exists yet
    // and no assistant turns have started. If the process is running but no
    // interaction has started after a short delay, the CLI is likely waiting
    // for trust confirmation.
    let has_user_message = recent_events
        .iter()
        .any(|e| e.get("type").and_then(|v| v.as_str()) == Some("user.message"));
    let has_agent_activity = recent_events.iter().any(|e| {
        let t = e.get("type").and_then(|v| v.as_str()).unwrap_or("");
        matches!(
            t,
            "assistant.turn_start"
                | "assistant.message"
                | "assistant.turn_end"
                | "tool.execution_start"
                | "tool.execution_complete"
                | "session.compaction_start"
                | "session.compaction_complete"
        )
    });
    if process_running
        && !has_user_message
        && !has_agent_activity
        && last_event_age_secs > WORKSPACE_TRUST_THRESHOLD_SECS
    {
        return SessionActivity::InputNeeded;
    }

    match event_type {
        // Agent is actively generating/working
        "assistant.turn_start" | "assistant.message" | "tool.execution_start"
        | "tool.execution_complete" | "subagent.started" | "subagent.completed"
        | "session.mode_changed" | "session.context_changed"
        | "session.compaction_start" | "session.compaction_complete" => {
            if is_stale {
                SessionActivity::Idle
            } else {
                SessionActivity::InProgress
            }
        }

        // Agent finished a turn — treat as idle
        "assistant.turn_end" => SessionActivity::Idle,

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn now_ts() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    /// Create a timestamp N seconds in the past.
    fn past_ts(seconds_ago: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::seconds(seconds_ago)).to_rfc3339()
    }

    fn make_event(event_type: &str, ts: &str) -> serde_json::Value {
        json!({"type": event_type, "timestamp": ts, "data": {}})
    }

    fn make_tool_start(tool_name: &str, ts: &str) -> serde_json::Value {
        json!({
            "type": "tool.execution_start",
            "timestamp": ts,
            "data": {"toolCallId": "tc1", "toolName": tool_name}
        })
    }

    // ---- ask_user detection (unchanged) ----

    #[test]
    fn ask_user_tool_is_input_needed() {
        let events = vec![
            make_event("assistant.turn_start", &now_ts()),
            make_tool_start("ask_user", &now_ts()),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::InputNeeded);
    }

    #[test]
    fn ask_user_stale_no_process_is_idle() {
        let events = vec![
            make_event("assistant.turn_start", &past_ts(300)),
            make_tool_start("ask_user", &past_ts(300)),
        ];
        assert_eq!(classify_events(&events, false), SessionActivity::Idle);
    }

    // ---- Tool confirmation heuristic ----

    #[test]
    fn active_turn_old_event_is_input_needed() {
        // Simulates "Do you want to run this command?" — turn started >60s ago,
        // last event is tool.execution_complete from previous step, process running.
        let events = vec![
            make_event("user.message", &past_ts(90)),
            make_event("assistant.turn_start", &past_ts(85)),
            make_event("assistant.message", &past_ts(80)),
            make_tool_start("view", &past_ts(80)),
            make_event("tool.execution_complete", &past_ts(75)),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::InputNeeded);
    }

    #[test]
    fn active_turn_recent_event_is_in_progress() {
        // Same as above but events are recent — model is still thinking
        let events = vec![
            make_event("user.message", &now_ts()),
            make_event("assistant.turn_start", &now_ts()),
            make_event("assistant.message", &now_ts()),
            make_tool_start("view", &now_ts()),
            make_event("tool.execution_complete", &now_ts()),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::InProgress);
    }

    #[test]
    fn turn_start_old_is_input_needed() {
        // turn_start >60s ago with no further events — likely a confirmation prompt
        let events = vec![
            make_event("user.message", &past_ts(90)),
            make_event("assistant.turn_start", &past_ts(70)),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::InputNeeded);
    }

    #[test]
    fn turn_start_recent_is_in_progress() {
        // turn_start just happened — model is thinking
        let events = vec![
            make_event("user.message", &now_ts()),
            make_event("assistant.turn_start", &now_ts()),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::InProgress);
    }

    #[test]
    fn active_turn_tool_running_not_input_needed() {
        // tool.execution_start is last event — tool is actively running, not confirmation.
        // The confirmation heuristic skips tool.execution_start events.
        let events = vec![
            make_event("user.message", &past_ts(130)),
            make_event("assistant.turn_start", &past_ts(120)),
            make_tool_start("bash", &past_ts(90)),
        ];
        // Not stale (90 < 120), so InProgress. Caller maps to "closed" if process dead.
        assert_eq!(classify_events(&events, true), SessionActivity::InProgress);
    }

    #[test]
    fn turn_ended_is_idle() {
        let events = vec![
            make_event("assistant.turn_start", &now_ts()),
            make_event("assistant.message", &now_ts()),
            make_tool_start("bash", &now_ts()),
            make_event("tool.execution_complete", &now_ts()),
            make_event("assistant.turn_end", &now_ts()),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::Idle);
    }

    #[test]
    fn no_process_active_turn_old_not_input_needed() {
        // Same scenario as confirmation prompt but process not running →
        // heuristic doesn't trigger (requires process_running).
        // Event is recent (65s < 120s) so match returns InProgress.
        // Caller maps to "closed" when process is not running.
        let events = vec![
            make_event("assistant.turn_start", &past_ts(70)),
            make_event("tool.execution_complete", &past_ts(65)),
        ];
        assert_eq!(
            classify_events(&events, false),
            SessionActivity::InProgress
        );
    }

    // ---- Workspace trust heuristic ----

    #[test]
    fn session_start_no_user_message_is_input_needed() {
        // Process running, session.start >15s ago, no user.message → trust prompt
        let events = vec![make_event("session.start", &past_ts(20))];
        assert_eq!(classify_events(&events, true), SessionActivity::InputNeeded);
    }

    #[test]
    fn session_start_no_user_message_recent_is_in_progress() {
        // Process running, session.start <15s ago → still initializing
        let events = vec![make_event("session.start", &now_ts())];
        assert_eq!(classify_events(&events, true), SessionActivity::InProgress);
    }

    #[test]
    fn session_start_with_user_message_is_in_progress() {
        // Process running, user.message exists → not a trust prompt
        let events = vec![
            make_event("session.start", &past_ts(20)),
            make_event("user.message", &now_ts()),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::InProgress);
    }

    #[test]
    fn session_start_no_process_not_input_needed() {
        // No process running → trust heuristic doesn't trigger.
        // Event is recent (20s < 120s) so match returns InProgress.
        // Caller maps to "closed" when process is not running.
        let events = vec![make_event("session.start", &past_ts(20))];
        assert_eq!(
            classify_events(&events, false),
            SessionActivity::InProgress
        );
    }

    // ---- General tests ----

    #[test]
    fn task_complete_is_idle() {
        let events = vec![
            make_event("assistant.turn_start", &now_ts()),
            make_tool_start("task_complete", &now_ts()),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::Idle);
    }

    // ---- Compaction / checkpoint events ----

    #[test]
    fn compaction_start_is_in_progress() {
        let events = vec![
            make_event("assistant.turn_start", &now_ts()),
            make_event("session.compaction_start", &now_ts()),
        ];
        assert_eq!(
            classify_events(&events, true),
            SessionActivity::InProgress
        );
    }

    #[test]
    fn compaction_complete_is_in_progress() {
        let events = vec![
            make_event("assistant.turn_start", &now_ts()),
            make_event("session.compaction_complete", &now_ts()),
        ];
        assert_eq!(
            classify_events(&events, true),
            SessionActivity::InProgress
        );
    }

    #[test]
    fn compaction_complete_stale_is_idle() {
        let events = vec![
            make_event("user.message", &past_ts(600)),
            make_event("session.compaction_complete", &past_ts(300)),
        ];
        assert_eq!(classify_events(&events, true), SessionActivity::Idle);
    }
}
