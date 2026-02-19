use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use tokio::task;

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
    let run = match fetch_workflow_run_via_http(token, owner, repo, run_id).await {
        Ok(run) => run,
        Err(http_err) => match fetch_workflow_run_via_gh(token, owner, repo, run_id).await {
            Ok(run) => run,
            Err(gh_err) => {
                return Err(anyhow::anyhow!(
                    "GitHub polling failed via HTTP and gh CLI | http: {} | gh: {}",
                    http_err,
                    gh_err
                ));
            }
        },
    };

    let mut result = HashMap::new();
    result.insert("status".to_string(), serde_json::json!(run.status));
    result.insert("conclusion".to_string(), serde_json::json!(run.conclusion));
    result.insert("name".to_string(), serde_json::json!(run.name));
    result.insert("updated_at".to_string(), serde_json::json!(run.updated_at));

    Ok(result)
}

async fn fetch_workflow_run_via_http(
    token: &str,
    owner: &str,
    repo: &str,
    run_id: &str,
) -> Result<WorkflowRun> {
    if token.trim().is_empty() {
        return Err(anyhow::anyhow!("GitHub token not configured"));
    }

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
        let status = response.status();
        let sso_header = response
            .headers()
            .get("x-github-sso")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.to_string());
        let body = response.text().await.unwrap_or_default();
        let mut message = format!("GitHub API error: {}", status);
        if !body.trim().is_empty() {
            message.push_str(&format!(" | {}", body));
        }
        if let Some(sso) = sso_header {
            message.push_str(&format!(" | x-github-sso: {}", sso));
        }
        return Err(anyhow::anyhow!(message));
    }

    Ok(response.json().await?)
}

async fn fetch_workflow_run_via_gh(
    token: &str,
    owner: &str,
    repo: &str,
    run_id: &str,
) -> Result<WorkflowRun> {
    let endpoint = format!("repos/{}/{}/actions/runs/{}", owner, repo, run_id);
    let endpoint_with_token = endpoint.clone();
    let token_owned = token.to_string();

    let with_token = task::spawn_blocking(move || {
        run_gh_api(&endpoint_with_token, (!token_owned.trim().is_empty()).then_some(token_owned.as_str()))
    })
    .await??;

    match with_token {
        Ok(body) => return Ok(serde_json::from_str::<WorkflowRun>(&body)?),
        Err(err_with_token) => {
            let endpoint_no_token = endpoint.clone();
            let without_token = task::spawn_blocking(move || run_gh_api(&endpoint_no_token, None)).await??;
            match without_token {
                Ok(body) => Ok(serde_json::from_str::<WorkflowRun>(&body)?),
                Err(err_no_token) => Err(anyhow::anyhow!(
                    "gh api failed with token and with local auth | with token: {} | local auth: {}",
                    err_with_token,
                    err_no_token
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
        Ok(Err(String::from_utf8_lossy(&output.stderr).trim().to_string()))
    }
}
