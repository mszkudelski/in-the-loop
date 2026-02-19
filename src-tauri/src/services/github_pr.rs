use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use tokio::task;

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
    let (pr, reviews) = match fetch_pr_via_http(token, owner, repo, pr_number).await {
        Ok(tuple) => tuple,
        Err(http_err) => match fetch_pr_via_gh(token, owner, repo, pr_number).await {
            Ok(tuple) => tuple,
            Err(gh_err) => {
                return Err(anyhow::anyhow!(
                    "GitHub PR polling failed via HTTP and gh CLI | http: {} | gh: {}",
                    http_err,
                    gh_err
                ));
            }
        },
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

async fn fetch_pr_via_http(
    token: &str,
    owner: &str,
    repo: &str,
    pr_number: &str,
) -> Result<(PullRequest, Vec<Review>)> {
    if token.trim().is_empty() {
        return Err(anyhow::anyhow!("GitHub token not configured"));
    }

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
        let status = pr_response.status();
        let sso_header = pr_response
            .headers()
            .get("x-github-sso")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let body = pr_response.text().await.unwrap_or_default();
        let mut message = format!("GitHub API error: {}", status);
        if !body.trim().is_empty() {
            message.push_str(&format!(" | {}", body));
        }
        if let Some(sso) = sso_header {
            message.push_str(&format!(" | x-github-sso: {}", sso));
        }
        return Err(anyhow::anyhow!(message));
    }

    let pr: PullRequest = pr_response.json().await?;

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

    Ok((pr, reviews))
}

async fn fetch_pr_via_gh(
    token: &str,
    owner: &str,
    repo: &str,
    pr_number: &str,
) -> Result<(PullRequest, Vec<Review>)> {
    let pr_endpoint = format!("repos/{}/{}/pulls/{}", owner, repo, pr_number);
    let reviews_endpoint = format!("repos/{}/{}/pulls/{}/reviews", owner, repo, pr_number);
    let token_owned = token.to_string();
    let pr_endpoint_with_token = pr_endpoint.clone();
    let reviews_endpoint_with_token = reviews_endpoint.clone();

    let with_token = task::spawn_blocking(move || {
        let pr = run_gh_api(&pr_endpoint_with_token, (!token_owned.trim().is_empty()).then_some(token_owned.as_str()))?;
        let reviews = run_gh_api(&reviews_endpoint_with_token, (!token_owned.trim().is_empty()).then_some(token_owned.as_str()))?;
        Ok::<_, anyhow::Error>((pr, reviews))
    })
    .await??;

    match with_token {
        (Ok(pr_body), Ok(reviews_body)) => Ok((
            serde_json::from_str::<PullRequest>(&pr_body)?,
            serde_json::from_str::<Vec<Review>>(&reviews_body)?,
        )),
        (pr_result, reviews_result) => {
            let pr_endpoint_no_token = pr_endpoint.clone();
            let reviews_endpoint_no_token = reviews_endpoint.clone();
            let without_token = task::spawn_blocking(move || {
                let pr = run_gh_api(&pr_endpoint_no_token, None)?;
                let reviews = run_gh_api(&reviews_endpoint_no_token, None)?;
                Ok::<_, anyhow::Error>((pr, reviews))
            })
            .await??;

            match without_token {
                (Ok(pr_body), Ok(reviews_body)) => Ok((
                    serde_json::from_str::<PullRequest>(&pr_body)?,
                    serde_json::from_str::<Vec<Review>>(&reviews_body)?,
                )),
                (pr_no_token, reviews_no_token) => {
                    let with_token_err = format!(
                        "pr: {} | reviews: {}",
                        pr_result.err().unwrap_or_else(|| "none".to_string()),
                        reviews_result.err().unwrap_or_else(|| "none".to_string())
                    );
                    let no_token_err = format!(
                        "pr: {} | reviews: {}",
                        pr_no_token.err().unwrap_or_else(|| "none".to_string()),
                        reviews_no_token.err().unwrap_or_else(|| "none".to_string())
                    );
                    Err(anyhow::anyhow!(
                        "gh api failed with token and with local auth | with token: {} | local auth: {}",
                        with_token_err,
                        no_token_err
                    ))
                }
            }
        }
    }
}

fn run_gh_api(endpoint: &str, token: Option<&str>) -> Result<std::result::Result<String, String>> {
    let mut command = Command::new("gh");
    command
        .arg("api")
        .arg(endpoint)
        .arg("-H")
        .arg("Accept: application/vnd.github+json")
        .arg("-H")
        .arg("X-GitHub-Api-Version: 2022-11-28");

    if let Some(token) = token {
        command.env("GH_TOKEN", token);
    }

    let output = match command.output() {
        Ok(output) => output,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(anyhow::anyhow!("GitHub CLI not found (`gh` not installed)"));
            }
            return Err(anyhow::anyhow!(e));
        }
    };

    if output.status.success() {
        Ok(Ok(String::from_utf8_lossy(&output.stdout).to_string()))
    } else {
        Ok(Err(String::from_utf8_lossy(&output.stderr).trim().to_string()))
    }
}
