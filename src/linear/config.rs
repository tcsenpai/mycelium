use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::error::{MyceliumError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearConfig {
    pub api_key: String,
    pub team_id: String,
    pub team_name: String,
    pub sync_enabled: bool,
    /// How to derive the repo label. "auto" = git remote name, or a fixed string.
    #[serde(default = "default_repo_label_mode")]
    pub repo_label: String,
    /// How to derive the branch label. "auto" = git branch name, or a fixed string.
    #[serde(default = "default_branch_label_mode")]
    pub branch_label: String,
    /// Extra static labels to always filter by (optional)
    #[serde(default)]
    pub extra_labels: Vec<String>,
    /// Default Linear user ID to assign issues to (auto-detected from git user)
    #[serde(default)]
    pub default_assignee_id: Option<String>,
    #[serde(default)]
    pub mapping: MappingConfig,
}

fn default_repo_label_mode() -> String {
    "auto".to_string()
}

fn default_branch_label_mode() -> String {
    "auto".to_string()
}

impl LinearConfig {
    /// Resolve the actual filter labels at runtime using git context.
    pub fn resolve_filter_labels(&self) -> Vec<String> {
        let mut labels = Vec::new();

        // Resolve repo label
        let repo = if self.repo_label == "auto" {
            detect_repo_name()
        } else if self.repo_label.is_empty() || self.repo_label == "none" {
            None
        } else {
            Some(self.repo_label.clone())
        };
        if let Some(r) = repo {
            labels.push(r);
        }

        // Resolve branch label
        let branch = if self.branch_label == "auto" {
            detect_branch_name()
        } else if self.branch_label.is_empty() || self.branch_label == "none" {
            None
        } else {
            Some(self.branch_label.clone())
        };
        if let Some(b) = branch {
            labels.push(b);
        }

        // Add any static extra labels
        labels.extend(self.extra_labels.iter().cloned());

        labels
    }
}

/// Detect repo name from git remote origin URL or folder name.
fn detect_repo_name() -> Option<String> {
    // Try git remote
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    let url = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if !url.is_empty() {
        // Extract repo name from URL: git@github.com:org/repo.git or https://github.com/org/repo.git
        let name = url
            .rsplit('/')
            .next()
            .unwrap_or(&url)
            .trim_end_matches(".git")
            .to_string();
        if !name.is_empty() {
            return Some(name);
        }
    }
    // Fallback: folder name
    std::env::current_dir()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// Detect current git branch name.
fn detect_branch_name() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if branch.is_empty() || branch == "HEAD" {
        None
    } else {
        Some(branch)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MappingConfig {
    #[serde(default = "default_epic_mode")]
    pub epic_mode: String,
    #[serde(default = "default_status_mapping")]
    pub status: HashMap<String, String>,
    #[serde(default = "default_priority_mapping")]
    pub priority: HashMap<String, u8>,
    #[serde(default)]
    pub assignees: HashMap<String, String>, // local assignee email/name → Linear user ID
}

impl Default for MappingConfig {
    fn default() -> Self {
        Self {
            epic_mode: default_epic_mode(),
            status: default_status_mapping(),
            priority: default_priority_mapping(),
            assignees: HashMap::new(),
        }
    }
}

fn default_epic_mode() -> String {
    "label".to_string()
}

fn default_status_mapping() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("open".to_string(), "Todo".to_string());
    m.insert("closed".to_string(), "Done".to_string());
    m
}

fn default_priority_mapping() -> HashMap<String, u8> {
    let mut m = HashMap::new();
    m.insert("low".to_string(), 4);
    m.insert("medium".to_string(), 3);
    m.insert("high".to_string(), 2);
    m.insert("critical".to_string(), 1);
    m
}

impl LinearConfig {
    pub fn config_dir() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".mycelium")
            .join(".linear")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Err(MyceliumError::LinearConfig(
                "Linear not configured. Run `myc linear setup` first".to_string(),
            ));
        }
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content).map_err(|e| MyceliumError::LinearConfig(e.to_string()))
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;

        // Ensure .linear is in .gitignore (contains API key)
        let gitignore_path = dir.parent().unwrap().join(".gitignore");
        if gitignore_path.exists() {
            let content = std::fs::read_to_string(&gitignore_path)?;
            if !content.contains(".linear") {
                let mut f = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&gitignore_path)?;
                use std::io::Write;
                writeln!(f, ".linear/")?;
            }
        } else {
            std::fs::write(&gitignore_path, ".linear/\n")?;
        }

        let content =
            toml::to_string_pretty(self).map_err(|e| MyceliumError::LinearConfig(e.to_string()))?;
        std::fs::write(Self::config_path(), content)?;
        Ok(())
    }

    pub fn exists() -> bool {
        Self::config_path().exists()
    }

    pub fn remove() -> Result<()> {
        let dir = Self::config_dir();
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }
}
