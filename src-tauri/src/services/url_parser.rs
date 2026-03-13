use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedUrl {
    pub item_type: String,
    pub metadata: HashMap<String, String>,
    pub suggested_title: String,
}

pub fn parse_url(url: &str) -> Result<ParsedUrl> {
    // Slack thread: *.slack.com/archives/CHANNEL/pTIMESTAMP
    let slack_regex = Regex::new(r"https?://[^/]+\.slack\.com/archives/([^/]+)/p(\d+)")?;
    if let Some(captures) = slack_regex.captures(url) {
        let channel_id = captures.get(1).unwrap().as_str();
        let thread_ts = captures.get(2).unwrap().as_str();

        // Convert pXXXXXXXXXX to XXX.XXXXXXX format
        let ts = if thread_ts.len() >= 10 {
            format!("{}.{}", &thread_ts[0..10], &thread_ts[10..])
        } else {
            thread_ts.to_string()
        };

        let mut metadata = HashMap::new();
        metadata.insert("channel_id".to_string(), channel_id.to_string());
        metadata.insert("thread_ts".to_string(), ts);

        return Ok(ParsedUrl {
            item_type: "slack_thread".to_string(),
            metadata,
            suggested_title: format!("Slack thread in {}", channel_id),
        });
    }

    // GitHub Action: github.com/OWNER/REPO/actions/runs/ID
    let gh_action_regex = Regex::new(r"https?://github\.com/([^/]+)/([^/]+)/actions/runs/(\d+)")?;
    if let Some(captures) = gh_action_regex.captures(url) {
        let owner = captures.get(1).unwrap().as_str();
        let repo = captures.get(2).unwrap().as_str();
        let run_id = captures.get(3).unwrap().as_str();

        let mut metadata = HashMap::new();
        metadata.insert("owner".to_string(), owner.to_string());
        metadata.insert("repo".to_string(), repo.to_string());
        metadata.insert("run_id".to_string(), run_id.to_string());

        return Ok(ParsedUrl {
            item_type: "github_action".to_string(),
            metadata,
            suggested_title: format!("GitHub Action: {}/{} #{}", owner, repo, run_id),
        });
    }

    // GitHub PR: github.com/OWNER/REPO/pull/NUMBER
    let gh_pr_regex = Regex::new(r"https?://github\.com/([^/]+)/([^/]+)/pull/(\d+)")?;
    if let Some(captures) = gh_pr_regex.captures(url) {
        let owner = captures.get(1).unwrap().as_str();
        let repo = captures.get(2).unwrap().as_str();
        let pr_number = captures.get(3).unwrap().as_str();

        let mut metadata = HashMap::new();
        metadata.insert("owner".to_string(), owner.to_string());
        metadata.insert("repo".to_string(), repo.to_string());
        metadata.insert("pr_number".to_string(), pr_number.to_string());

        return Ok(ParsedUrl {
            item_type: "github_pr".to_string(),
            metadata,
            suggested_title: format!("PR: {}/{} #{}", owner, repo, pr_number),
        });
    }

    // GitHub repository — must be AFTER action/PR patterns (more specific first)
    // Also matches URLs with trailing sub-paths like /pulls, /issues, /wiki, etc.
    let github_repo_re = Regex::new(
        r"https?://github\.com/([^/]+)/([^/]+?)(?:/(?:pulls|issues|wiki|projects|actions|security|pulse|graphs|network|settings))?/?$",
    )?;
    if let Some(caps) = github_repo_re.captures(url) {
        let owner = caps[1].to_string();
        let repo = caps[2].to_string();
        let suggested_title = format!("{}/{}", owner, repo);
        let mut metadata = HashMap::new();
        metadata.insert("owner".to_string(), owner);
        metadata.insert("repo".to_string(), repo);
        return Ok(ParsedUrl {
            item_type: "github_repo".to_string(),
            metadata,
            suggested_title,
        });
    }

    Err(anyhow!("Unsupported URL format. Expected Slack thread, GitHub Action, GitHub PR, or GitHub repository URL."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slack_url() {
        let url = "https://myworkspace.slack.com/archives/C12345678/p1234567890123456";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "slack_thread");
        assert_eq!(result.metadata.get("channel_id").unwrap(), "C12345678");
        assert_eq!(
            result.metadata.get("thread_ts").unwrap(),
            "1234567890.123456"
        );
    }

    #[test]
    fn test_parse_github_action_url() {
        let url = "https://github.com/owner/repo/actions/runs/12345678";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "github_action");
        assert_eq!(result.metadata.get("owner").unwrap(), "owner");
        assert_eq!(result.metadata.get("repo").unwrap(), "repo");
        assert_eq!(result.metadata.get("run_id").unwrap(), "12345678");
    }

    #[test]
    fn test_parse_github_pr_url() {
        let url = "https://github.com/owner/repo/pull/42";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "github_pr");
        assert_eq!(result.metadata.get("owner").unwrap(), "owner");
        assert_eq!(result.metadata.get("repo").unwrap(), "repo");
        assert_eq!(result.metadata.get("pr_number").unwrap(), "42");
    }

    #[test]
    fn test_parse_github_repo_url() {
        let url = "https://github.com/facebook/react";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "github_repo");
        assert_eq!(result.metadata.get("owner").unwrap(), "facebook");
        assert_eq!(result.metadata.get("repo").unwrap(), "react");
        assert_eq!(result.suggested_title, "facebook/react");
    }

    #[test]
    fn test_parse_github_repo_url_with_trailing_slash() {
        let url = "https://github.com/facebook/react/";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "github_repo");
        assert_eq!(result.metadata.get("owner").unwrap(), "facebook");
        assert_eq!(result.metadata.get("repo").unwrap(), "react");
    }

    #[test]
    fn test_github_pr_url_not_matched_as_repo() {
        let url = "https://github.com/owner/repo/pull/42";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "github_pr");
    }

    #[test]
    fn test_parse_github_repo_url_with_pulls_suffix() {
        let url = "https://github.com/facebook/react/pulls";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "github_repo");
        assert_eq!(result.metadata.get("owner").unwrap(), "facebook");
        assert_eq!(result.metadata.get("repo").unwrap(), "react");
        assert_eq!(result.suggested_title, "facebook/react");
    }

    #[test]
    fn test_parse_github_repo_url_with_issues_suffix() {
        let url = "https://github.com/facebook/react/issues";
        let result = parse_url(url).unwrap();
        assert_eq!(result.item_type, "github_repo");
        assert_eq!(result.metadata.get("owner").unwrap(), "facebook");
        assert_eq!(result.metadata.get("repo").unwrap(), "react");
    }
}
