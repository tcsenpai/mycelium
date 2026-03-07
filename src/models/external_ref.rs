use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use super::ExternalRefType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRef {
    pub id: i64,
    pub task_id: i64,
    pub ref_type: ExternalRefType,
    pub reference: String,
    pub created_at: DateTime<Local>,
}

impl ExternalRef {
    pub fn new(id: i64, task_id: i64, ref_type: ExternalRefType, reference: impl Into<String>) -> Self {
        Self {
            id,
            task_id,
            ref_type,
            reference: reference.into(),
            created_at: Local::now(),
        }
    }

    pub fn github_issue(task_id: i64, owner: &str, repo: &str, number: i64) -> Self {
        Self::new(
            0,
            task_id,
            ExternalRefType::GitHubIssue,
            format!("{}/{}/{}", owner, repo, number),
        )
    }

    pub fn github_pr(task_id: i64, owner: &str, repo: &str, number: i64) -> Self {
        Self::new(
            0,
            task_id,
            ExternalRefType::GitHubPr,
            format!("{}/{}/{}", owner, repo, number),
        )
    }

    pub fn url(task_id: i64, url: impl Into<String>) -> Self {
        Self::new(0, task_id, ExternalRefType::Url, url)
    }
}

pub fn parse_github_ref(ref_str: &str) -> Option<(String, String, i64)> {
    use regex::Regex;
    let re = Regex::new(r"^([^/]+)/([^/]+)#(\d+)$").ok()?;
    let caps = re.captures(ref_str)?;
    let owner = caps.get(1)?.as_str().to_string();
    let repo = caps.get(2)?.as_str().to_string();
    let number = caps.get(3)?.as_str().parse::<i64>().ok()?;
    Some((owner, repo, number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_ref() {
        assert_eq!(
            parse_github_ref("owner/repo#123"),
            Some(("owner".to_string(), "repo".to_string(), 123))
        );
        assert_eq!(
            parse_github_ref("my-org/my-repo#42"),
            Some(("my-org".to_string(), "my-repo".to_string(), 42))
        );
        assert_eq!(parse_github_ref("invalid"), None);
        assert_eq!(parse_github_ref("owner/repo"), None);
    }
}
