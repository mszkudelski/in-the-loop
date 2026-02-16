use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct WorkflowRun {
    id: u64,
    name: String,
    status: String,
    conclusion: Option<String>,
    created_at: String,
    updated_at: String,
}

pub async fn check_github_action(
    token: &str,
    owner: &str,
    repo: &str,
    run_id: &str,
) -> Result<HashMap<String, serde_json::Value>> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/{}/actions/runs/{}",
        owner, repo, run_id
    );

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "in-the-loop-app")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "GitHub API error: {}",
            response.status()
        ));
    }

    let run: WorkflowRun = response.json().await?;

    let mut result = HashMap::new();
    result.insert("status".to_string(), serde_json::json!(run.status));
    result.insert("conclusion".to_string(), serde_json::json!(run.conclusion));
    result.insert("name".to_string(), serde_json::json!(run.name));
    result.insert("updated_at".to_string(), serde_json::json!(run.updated_at));

    Ok(result)
}
