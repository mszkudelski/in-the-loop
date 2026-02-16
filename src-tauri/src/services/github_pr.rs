use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
struct PullRequest {
    number: u64,
    title: String,
    state: String,
    merged: bool,
    draft: bool,
    updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Review {
    state: String,
    submitted_at: Option<String>,
}

pub async fn check_github_pr(
    token: &str,
    owner: &str,
    repo: &str,
    pr_number: &str,
) -> Result<HashMap<String, serde_json::Value>> {
    let client = reqwest::Client::new();
    let pr_url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}",
        owner, repo, pr_number
    );

    let pr_response = client
        .get(&pr_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "in-the-loop-app")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    if !pr_response.status().is_success() {
        return Err(anyhow::anyhow!(
            "GitHub API error: {}",
            pr_response.status()
        ));
    }

    let pr: PullRequest = pr_response.json().await?;

    // Get reviews
    let reviews_url = format!(
        "https://api.github.com/repos/{}/{}/pulls/{}/reviews",
        owner, repo, pr_number
    );

    let reviews_response = client
        .get(&reviews_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "in-the-loop-app")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?;

    let reviews: Vec<Review> = if reviews_response.status().is_success() {
        reviews_response.json().await?
    } else {
        Vec::new()
    };

    let mut result = HashMap::new();
    result.insert("title".to_string(), serde_json::json!(pr.title));
    result.insert("state".to_string(), serde_json::json!(pr.state));
    result.insert("merged".to_string(), serde_json::json!(pr.merged));
    result.insert("draft".to_string(), serde_json::json!(pr.draft));
    result.insert("updated_at".to_string(), serde_json::json!(pr.updated_at));
    result.insert("review_count".to_string(), serde_json::json!(reviews.len()));
    
    // Check for approval or changes requested
    let has_approval = reviews.iter().any(|r| r.state == "APPROVED");
    let has_changes_requested = reviews.iter().any(|r| r.state == "CHANGES_REQUESTED");
    
    result.insert("has_approval".to_string(), serde_json::json!(has_approval));
    result.insert("has_changes_requested".to_string(), serde_json::json!(has_changes_requested));

    Ok(result)
}
