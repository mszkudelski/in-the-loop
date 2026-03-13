use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;
use tokio::task;

// TODO: use shared GITHUB_API_BASE constant once refactor-github-api-constants is done
const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
struct RepoPullRequest {
    number: u64,
    title: String,
    user: Option<PrUser>,
    draft: Option<bool>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PrUser {
    login: String,
}

pub async fn check_github_repo_prs(
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<HashMap<String, serde_json::Value>> {
    let prs = match fetch_prs_via_http(token, owner, repo).await {
        Ok(prs) => prs,
        Err(http_err) => match fetch_prs_via_gh(token, owner, repo).await {
            Ok(prs) => prs,
            Err(gh_err) => {
                return Err(anyhow::anyhow!(
                    "GitHub repo PR polling failed via HTTP and gh CLI | http: {} | gh: {}",
                    http_err,
                    gh_err
                ));
            }
        },
    };

    let open_prs: Vec<serde_json::Value> = prs
        .iter()
        .map(|pr| {
            serde_json::json!({
                "number": pr.number,
                "title": pr.title,
                "author": pr.user.as_ref().map(|u| u.login.as_str()).unwrap_or("unknown"),
                "draft": pr.draft.unwrap_or(false),
                "updated_at": pr.updated_at.as_deref().unwrap_or(""),
            })
        })
        .collect();

    let mut result = HashMap::new();
    result.insert(
        "open_pr_count".to_string(),
        serde_json::json!(open_prs.len()),
    );
    result.insert("open_prs".to_string(), serde_json::json!(open_prs));

    Ok(result)
}

async fn fetch_prs_via_http(
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<RepoPullRequest>> {
    if token.trim().is_empty() {
        return Err(anyhow::anyhow!("GitHub token not configured"));
    }

    let client = reqwest::Client::new();
    let url = format!(
        "{}/repos/{}/{}/pulls?state=open&per_page=100",
        GITHUB_API_BASE, owner, repo
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
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("GitHub API error: {} | {}", status, body));
    }

    let prs: Vec<RepoPullRequest> = response.json().await?;
    Ok(prs)
}

async fn fetch_prs_via_gh(
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<RepoPullRequest>> {
    let endpoint = format!(
        "repos/{}/{}/pulls?state=open&per_page=100",
        owner, repo
    );
    let token_owned = token.to_string();
    let endpoint_clone = endpoint.clone();

    let with_token = task::spawn_blocking(move || {
        run_gh_api(
            &endpoint_clone,
            (!token_owned.trim().is_empty()).then_some(token_owned.as_str()),
        )
    })
    .await??;

    match with_token {
        Ok(body) => Ok(serde_json::from_str::<Vec<RepoPullRequest>>(&body)?),
        Err(_with_token_err) => {
            let endpoint_no_token = endpoint.clone();
            let without_token = task::spawn_blocking(move || {
                run_gh_api(&endpoint_no_token, None)
            })
            .await??;

            match without_token {
                Ok(body) => Ok(serde_json::from_str::<Vec<RepoPullRequest>>(&body)?),
                Err(no_token_err) => Err(anyhow::anyhow!(
                    "gh api failed with token and with local auth | with token: {} | local auth: {}",
                    _with_token_err,
                    no_token_err
                )),
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
        Ok(Err(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}
