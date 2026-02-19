use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenCodeSession {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub directory: String,
    #[serde(default, rename = "parentID")]
    pub parent_id: Option<String>,
    pub time: SessionTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionTime {
    pub created: f64,
    pub updated: f64,
    pub archived: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionStatus {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "busy")]
    Busy,
    #[serde(rename = "retry")]
    Retry {
        attempt: u32,
        message: String,
        next: f64,
    },
}

pub struct SessionMessageSummary {
    pub message_count: usize,
    pub total_tokens: u64,
    pub total_cost: f64,
    pub model: Option<String>,
    pub agent: Option<String>,
}

pub struct OpenCodeConfig {
    pub base_url: String,
    pub directory: Option<String>,
}

fn build_client() -> reqwest::Client {
    reqwest::Client::new()
}

fn build_request(client: &reqwest::Client, url: &str, password: &str) -> reqwest::RequestBuilder {
    let builder = client
        .get(url)
        .header("Accept", "application/json");
    if password.is_empty() {
        builder
    } else {
        builder.basic_auth("opencode", Some(password))
    }
}

pub fn parse_opencode_url(raw_url: &str) -> Result<OpenCodeConfig> {
    let parsed = Url::parse(raw_url)?;
    let base_url = parsed.origin().ascii_serialization();

    let segments: Vec<&str> = parsed
        .path_segments()
        .map(|s| s.filter(|seg| !seg.is_empty()).collect())
        .unwrap_or_default();

    let directory = segments.first().and_then(|seg| {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(seg)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    });

    Ok(OpenCodeConfig {
        base_url,
        directory,
    })
}

pub async fn list_sessions(
    base_url: &str,
    password: &str,
    directory: Option<&str>,
) -> Result<Vec<OpenCodeSession>> {
    let client = build_client();
    let url = format!("{}/session", base_url);

    let mut request = build_request(&client, &url, password);
    if let Some(dir) = directory {
        request = request.query(&[("directory", dir)]);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OpenCode API error (list_sessions): {} | {}",
            status,
            body
        ));
    }

    Ok(response.json().await?)
}

pub async fn get_session_statuses(
    base_url: &str,
    password: &str,
    directory: Option<&str>,
) -> Result<HashMap<String, SessionStatus>> {
    let client = build_client();
    let url = format!("{}/session/status", base_url);

    let mut request = build_request(&client, &url, password);
    if let Some(dir) = directory {
        request = request.query(&[("directory", dir)]);
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OpenCode API error (get_session_statuses): {} | {}",
            status,
            body
        ));
    }

    Ok(response.json().await?)
}

pub async fn get_session_message_summary(
    base_url: &str,
    password: &str,
    session_id: &str,
) -> Result<SessionMessageSummary> {
    let client = build_client();
    let url = format!("{}/session/{}/message", base_url, session_id);

    let response = build_request(&client, &url, password)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "OpenCode API error (get_session_message_summary): {} | {}",
            status,
            body
        ));
    }

    let messages: Vec<serde_json::Value> = response.json().await?;

    let mut message_count: usize = 0;
    let mut total_tokens: u64 = 0;
    let mut total_cost: f64 = 0.0;
    let mut model: Option<String> = None;
    let mut agent: Option<String> = None;

    for entry in &messages {
        let info = match entry.get("info") {
            Some(v) => v,
            None => continue,
        };

        message_count += 1;

        let role = info.get("role").and_then(|v| v.as_str()).unwrap_or("");

        if agent.is_none() {
            if let Some(a) = info.get("agent").and_then(|v| v.as_str()) {
                if !a.is_empty() {
                    agent = Some(a.to_string());
                }
            }
        }

        if role == "assistant" {
            if let Some(tokens) = info.get("tokens") {
                let input = tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0);
                let output = tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0);
                let reasoning = tokens.get("reasoning").and_then(|v| v.as_u64()).unwrap_or(0);
                total_tokens += input + output + reasoning;
            }

            if let Some(cost) = info.get("cost").and_then(|v| v.as_f64()) {
                total_cost += cost;
            }

            if let Some(m) = info.get("modelID").and_then(|v| v.as_str()) {
                if !m.is_empty() {
                    model = Some(m.to_string());
                }
            }
        }
    }

    Ok(SessionMessageSummary {
        message_count,
        total_tokens,
        total_cost,
        model,
        agent,
    })
}

pub fn enumerate_opencode_directories() -> Vec<String> {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return vec![],
    };
    let storage_path = std::path::PathBuf::from(&home)
        .join(".local/share/opencode/storage/session");
    let entries = match std::fs::read_dir(&storage_path) {
        Ok(e) => e,
        Err(_) => return vec![],
    };
    let mut directories = vec![];
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        if let Ok(files) = std::fs::read_dir(entry.path()) {
            for file in files.flatten() {
                let path = file.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(dir) = val["directory"].as_str() {
                                directories.push(dir.to_string());
                            }
                        }
                    }
                    break;
                }
            }
        }
    }
    directories
}

pub fn find_session_directory(session_id: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let storage_path = std::path::PathBuf::from(&home)
        .join(".local/share/opencode/storage/session");
    let entries = std::fs::read_dir(&storage_path).ok()?;

    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let session_file = entry.path().join(format!("{}.json", session_id));
        if session_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&session_file) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    return val["directory"].as_str().map(|s| s.to_string());
                }
            }
        }
    }
    None
}

pub fn build_web_url(base_url: &str, directory: &str) -> String {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(directory.as_bytes());
    format!("{}/{}", base_url, encoded)
}

pub async fn check_opencode_health(
    base_url: &str,
    password: &str,
) -> Result<bool> {
    let client = build_client();
    let url = format!("{}/global/health", base_url);

    let response = build_request(&client, &url, password)
        .send()
        .await;

    match response {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

pub async fn poll_opencode_session(
    base_url: &str,
    password: &str,
    session_id: &str,
    statuses: &HashMap<String, SessionStatus>,
) -> Result<HashMap<String, serde_json::Value>> {
    let summary = get_session_message_summary(base_url, password, session_id).await?;

    let status_str = match statuses.get(session_id) {
        Some(SessionStatus::Idle) => "idle",
        Some(SessionStatus::Busy) => "busy",
        Some(SessionStatus::Retry { .. }) => "retry",
        None => "unknown",
    };

    let mut result = HashMap::new();
    result.insert(
        "session_id".to_string(),
        serde_json::json!(session_id),
    );
    result.insert(
        "session_status".to_string(),
        serde_json::json!(status_str),
    );
    result.insert(
        "model".to_string(),
        serde_json::json!(summary.model),
    );
    result.insert(
        "agent".to_string(),
        serde_json::json!(summary.agent),
    );
    result.insert(
        "message_count".to_string(),
        serde_json::json!(summary.message_count),
    );
    result.insert(
        "total_tokens".to_string(),
        serde_json::json!(summary.total_tokens),
    );
    result.insert(
        "total_cost".to_string(),
        serde_json::json!(summary.total_cost),
    );

    Ok(result)
}
