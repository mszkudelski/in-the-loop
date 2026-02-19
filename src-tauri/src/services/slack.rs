use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct SlackResponse {
    ok: bool,
    messages: Option<Vec<SlackMessage>>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SlackMessage {
    #[serde(rename = "type")]
    msg_type: String,
    user: Option<String>,
    text: String,
    ts: String,
}

pub async fn check_slack_thread(
    token: &str,
    channel_id: &str,
    thread_ts: &str,
) -> Result<HashMap<String, serde_json::Value>> {
    let client = reqwest::Client::new();
    let url = "https://slack.com/api/conversations.replies";

    let response = client
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[("channel", channel_id), ("ts", thread_ts)])
        .send()
        .await?;

    let data: SlackResponse = response.json().await?;

    if !data.ok {
        return Err(anyhow::anyhow!(
            "Slack API error: {}",
            data.error.unwrap_or_else(|| "Unknown error".to_string())
        ));
    }

    let messages = data.messages.unwrap_or_default();
    let message_count = messages.len();
    let latest_ts = messages
        .last()
        .map(|m| m.ts.clone())
        .unwrap_or_else(|| thread_ts.to_string());

    let mut result = HashMap::new();
    result.insert(
        "message_count".to_string(),
        serde_json::json!(message_count),
    );
    result.insert("latest_ts".to_string(), serde_json::json!(latest_ts));
    result.insert(
        "messages".to_string(),
        serde_json::to_value(&messages).unwrap_or(serde_json::json!([])),
    );

    Ok(result)
}
