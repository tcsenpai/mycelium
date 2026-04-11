use colored::Colorize;
use std::io::Write;

use crate::cli::OutputFormat;
use crate::commands::{ensure_initialized, SUCCESS_PREFIX, ERROR_PREFIX, INFO_PREFIX, WARNING_PREFIX};
use crate::error::Result;
use crate::linear::client::LinearClient;
use crate::linear::config::LinearConfig;
use crate::linear::sync::{self, ConflictResolution};

pub fn setup(quiet: bool) -> Result<()> {
    if LinearConfig::exists() {
        if !quiet {
            println!("{} Linear is already configured.", WARNING_PREFIX);
            print!("  Reconfigure? [y/N] ");
            std::io::stdout().flush().ok();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            if !input.trim().eq_ignore_ascii_case("y") {
                return Ok(());
            }
        }
    }

    // Step 1: Get API key
    if !quiet {
        println!("\n{} Linear Integration Setup", INFO_PREFIX);
        println!("  Get your API key from: https://linear.app/settings/api\n");
    }
    print!("  Linear API key: ");
    std::io::stdout().flush().ok();
    let mut api_key = String::new();
    std::io::stdin().read_line(&mut api_key).ok();
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        return Err(crate::error::MyceliumError::LinearConfig("API key cannot be empty".to_string()));
    }

    // Step 2: Verify key and fetch teams
    if !quiet {
        println!("\n  Verifying API key...");
    }
    let client = LinearClient::new(&api_key);
    let teams = client.fetch_teams()?;

    if teams.is_empty() {
        return Err(crate::error::MyceliumError::LinearConfig("No teams found for this API key".to_string()));
    }

    // Step 3: Select team
    let (team_id, team_name) = if teams.len() == 1 {
        if !quiet {
            println!("  Found team: {} ({})", teams[0].name.green(), teams[0].key);
        }
        (teams[0].id.clone(), teams[0].name.clone())
    } else {
        if !quiet {
            println!("\n  Available teams:");
            for (i, team) in teams.iter().enumerate() {
                println!("    {}. {} ({})", i + 1, team.name, team.key);
            }
        }
        print!("\n  Select team [1-{}]: ", teams.len());
        std::io::stdout().flush().ok();
        let mut choice = String::new();
        std::io::stdin().read_line(&mut choice).ok();
        let idx: usize = choice.trim().parse().unwrap_or(1);
        let idx = idx.saturating_sub(1).min(teams.len() - 1);
        (teams[idx].id.clone(), teams[idx].name.clone())
    };

    // Step 4: Epic mapping mode
    if !quiet {
        println!("\n  How should epics be mapped to Linear?");
        println!("    1. Labels (default)");
        println!("    2. Projects");
    }
    print!("  Choice [1]: ");
    std::io::stdout().flush().ok();
    let mut epic_choice = String::new();
    std::io::stdin().read_line(&mut epic_choice).ok();
    let epic_mode = if epic_choice.trim() == "2" {
        "project".to_string()
    } else {
        "label".to_string()
    };

    // Step 5: Filter labels (compartmentalized sync — detected at runtime)
    // Auto-detect current values to show the user
    let detected_repo = crate::linear::config::LinearConfig::resolve_filter_labels(
        &crate::linear::config::LinearConfig {
            api_key: String::new(), team_id: String::new(), team_name: String::new(),
            sync_enabled: true, repo_label: "auto".to_string(), branch_label: "auto".to_string(),
            extra_labels: vec![], default_assignee_id: None, mapping: Default::default(),
        }
    );

    if !quiet {
        println!("\n  Sync filtering: issues are filtered by labels derived from git context.");
        println!("  This ensures each repo+branch only syncs its own issues.\n");
        if !detected_repo.is_empty() {
            println!("  Detected labels: {:?}", detected_repo);
        }
        println!("\n  Repo label source:");
        println!("    'auto' = derived from git remote name (default)");
        println!("    'none' = don't filter by repo");
        println!("    anything else = use that as a fixed label");
    }
    print!("  Repo label [auto]: ");
    std::io::stdout().flush().ok();
    let mut repo_input = String::new();
    std::io::stdin().read_line(&mut repo_input).ok();
    let repo_label = repo_input.trim().to_string();
    let repo_label = if repo_label.is_empty() { "auto".to_string() } else { repo_label };

    if !quiet {
        println!("\n  Branch label source:");
        println!("    'auto' = derived from current git branch (default)");
        println!("    'none' = don't filter by branch");
        println!("    anything else = use that as a fixed label");
    }
    print!("  Branch label [auto]: ");
    std::io::stdout().flush().ok();
    let mut branch_input = String::new();
    std::io::stdin().read_line(&mut branch_input).ok();
    let branch_label = branch_input.trim().to_string();
    let branch_label = if branch_label.is_empty() { "auto".to_string() } else { branch_label };

    // Extra static labels
    let mut extra_labels: Vec<String> = Vec::new();
    loop {
        print!("  Additional static label [leave empty to finish]: ");
        std::io::stdout().flush().ok();
        let mut extra = String::new();
        std::io::stdin().read_line(&mut extra).ok();
        let extra = extra.trim().to_string();
        if extra.is_empty() {
            break;
        }
        extra_labels.push(extra);
    }

    // Step 6: Assignee mapping — find YOUR Linear user
    if !quiet {
        println!("\n  Fetching team members for assignee mapping...");
    }
    let members = client.fetch_team_members(&team_id)?;
    let mut assignee_map = std::collections::HashMap::new();
    let mut default_assignee_id: Option<String> = None;

    // Try to auto-detect git user
    let git_email = std::process::Command::new("git")
        .args(["config", "user.email"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let git_name = std::process::Command::new("git")
        .args(["config", "user.name"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Also get git username from remote URL (github.com:USERNAME/repo)
    let git_username = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|url| {
            let url = url.trim();
            // git@github.com:user/repo.git
            if let Some(rest) = url.strip_prefix("git@github.com:") {
                rest.split('/').next().map(|s| s.to_string())
            // https://github.com/user/repo.git
            } else if url.contains("github.com/") {
                url.split("github.com/").nth(1)
                    .and_then(|s| s.split('/').next())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .filter(|s| !s.is_empty());

    if !members.is_empty() {
        if !quiet {
            println!("\n  Linear team members:");
            for (i, member) in members.iter().enumerate() {
                println!("    {}. {} ({})", i + 1, member.name, member.email);
            }
        }

        // Try matching: email > name > display_name > git username (case-insensitive, substring)
        let matched_member = None
            // Exact email match
            .or_else(|| git_email.as_ref().and_then(|email|
                members.iter().find(|m| m.email.eq_ignore_ascii_case(email))))
            // Exact name match
            .or_else(|| git_name.as_ref().and_then(|name|
                members.iter().find(|m|
                    m.name.eq_ignore_ascii_case(name) || m.display_name.eq_ignore_ascii_case(name))))
            // Git username match against name/displayName
            .or_else(|| git_username.as_ref().and_then(|username|
                members.iter().find(|m|
                    m.name.eq_ignore_ascii_case(username)
                    || m.display_name.eq_ignore_ascii_case(username)
                    || m.email.split('@').next().map_or(false, |local| local.eq_ignore_ascii_case(username)))))
            // Substring: git name contained in Linear name or vice versa
            .or_else(|| git_name.as_ref().and_then(|name| {
                let lower = name.to_lowercase();
                members.iter().find(|m|
                    m.name.to_lowercase().contains(&lower) || lower.contains(&m.name.to_lowercase())
                    || m.display_name.to_lowercase().contains(&lower) || lower.contains(&m.display_name.to_lowercase()))
            }));

        if let Some(member) = matched_member {
            default_assignee_id = Some(member.id.clone());
            if let Some(ref email) = git_email {
                assignee_map.insert(email.clone(), member.id.clone());
            }
            if let Some(ref name) = git_name {
                assignee_map.insert(name.clone(), member.id.clone());
            }
            if let Some(ref username) = git_username {
                assignee_map.insert(username.clone(), member.id.clone());
            }
            if !quiet {
                println!("\n  {} Matched you to: {} ({})", SUCCESS_PREFIX, member.name.green(), member.email);
            }
        } else {
            // No auto-match — ask user to pick
            if !quiet {
                println!("\n  Could not auto-detect your Linear user.");
                print!("  Select your user [1-{}, or 0 to skip]: ", members.len());
                std::io::stdout().flush().ok();
                let mut choice = String::new();
                std::io::stdin().read_line(&mut choice).ok();
                let idx: usize = choice.trim().parse().unwrap_or(0);
                if idx > 0 && idx <= members.len() {
                    let member = &members[idx - 1];
                    default_assignee_id = Some(member.id.clone());
                    if let Some(ref email) = git_email {
                        assignee_map.insert(email.clone(), member.id.clone());
                    }
                    if let Some(ref name) = git_name {
                        assignee_map.insert(name.clone(), member.id.clone());
                    }
                    println!("  {} Set default assignee to: {}", SUCCESS_PREFIX, member.name.green());
                }
            }
        }

        // Also map local assignees
        if let Ok(db) = ensure_initialized() {
            if let Ok(assignees) = db.list_assignees() {
                for assignee in &assignees {
                    if let Some(ref email) = assignee.email {
                        if let Some(member) = members.iter().find(|m| m.email.eq_ignore_ascii_case(email)) {
                            if !assignee_map.values().any(|v| v == &member.id) {
                                assignee_map.insert(email.clone(), member.id.clone());
                            }
                        }
                    }
                    if let Some(ref github) = assignee.github_username {
                        if let Some(member) = members.iter().find(|m| {
                            m.display_name.eq_ignore_ascii_case(github) || m.name.eq_ignore_ascii_case(github)
                        }) {
                            if !assignee_map.values().any(|v| v == &member.id) {
                                assignee_map.insert(github.clone(), member.id.clone());
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 7: Save config
    let config = LinearConfig {
        api_key,
        team_id,
        team_name: team_name.clone(),
        sync_enabled: true,
        repo_label,
        branch_label,
        extra_labels,
        default_assignee_id,
        mapping: crate::linear::config::MappingConfig {
            epic_mode,
            assignees: assignee_map,
            ..Default::default()
        },
    };
    config.save()?;

    if !quiet {
        println!("\n{} Linear integration configured for team '{}'", SUCCESS_PREFIX, team_name.green());
        println!("  Config saved to: {}", LinearConfig::config_path().display());
        println!("  Run `myc linear sync` to start syncing.");
    }

    Ok(())
}

pub fn sync_cmd(force_local: bool, force_remote: bool, quiet: bool) -> Result<()> {
    let config = LinearConfig::load()?;
    let mut db = ensure_initialized()?;
    let client = LinearClient::new(&config.api_key);

    let resolution = if force_local && force_remote {
        if !quiet {
            println!("{} Cannot use both --force-local and --force-remote. Defaulting to remote wins.", WARNING_PREFIX);
        }
        ConflictResolution::RemoteWins
    } else if force_local {
        ConflictResolution::LocalWins
    } else if force_remote {
        ConflictResolution::RemoteWins
    } else {
        ConflictResolution::RemoteWins
    };

    if !quiet {
        println!("{} Syncing with Linear (team: {})...", INFO_PREFIX, config.team_name);
    }

    let stats = sync::sync(&mut db, &client, &config, resolution, quiet)?;
    print_stats(&stats, quiet);
    Ok(())
}

pub fn push_cmd(quiet: bool) -> Result<()> {
    let config = LinearConfig::load()?;
    let mut db = ensure_initialized()?;
    let client = LinearClient::new(&config.api_key);

    if !quiet {
        println!("{} Pushing to Linear (team: {})...", INFO_PREFIX, config.team_name);
    }

    let stats = sync::push(&mut db, &client, &config, quiet)?;
    print_stats(&stats, quiet);
    Ok(())
}

pub fn pull_cmd(quiet: bool) -> Result<()> {
    let config = LinearConfig::load()?;
    let mut db = ensure_initialized()?;
    let client = LinearClient::new(&config.api_key);

    if !quiet {
        println!("{} Pulling from Linear (team: {})...", INFO_PREFIX, config.team_name);
    }

    let stats = sync::pull(&mut db, &client, &config, quiet)?;
    print_stats(&stats, quiet);
    Ok(())
}

pub fn status_cmd(format: &OutputFormat, quiet: bool) -> Result<()> {
    if !LinearConfig::exists() {
        if !quiet {
            println!("{} Linear is not configured. Run `myc linear setup`.", INFO_PREFIX);
        }
        return Ok(());
    }

    let config = LinearConfig::load()?;
    let db = ensure_initialized()?;

    let synced_count = db.count_linear_synced()?;
    let last_sync = db.get_last_sync_time()?;
    let resolved_labels = config.resolve_filter_labels();

    match format {
        OutputFormat::Json => {
            let status = serde_json::json!({
                "configured": true,
                "team_name": config.team_name,
                "team_id": config.team_id,
                "sync_enabled": config.sync_enabled,
                "epic_mode": config.mapping.epic_mode,
                "repo_label": config.repo_label,
                "branch_label": config.branch_label,
                "resolved_labels": resolved_labels,
                "synced_tasks": synced_count,
                "last_sync": last_sync,
                "assignee_mappings": config.mapping.assignees.len(),
            });
            if !quiet {
                println!("{}", serde_json::to_string_pretty(&status).unwrap());
            }
        }
        OutputFormat::Table => {
            if !quiet {
                println!("{} Linear Integration Status", INFO_PREFIX);
                println!("  Team:              {} ({})", config.team_name.green(), config.team_id);
                println!("  Sync enabled:      {}", if config.sync_enabled { "yes".green() } else { "no".red() });
                println!("  Epic mapping:      {}", config.mapping.epic_mode);
                println!("  Repo label:        {} ({})", config.repo_label, resolved_labels.first().unwrap_or(&"none".to_string()));
                println!("  Branch label:      {} ({})", config.branch_label, resolved_labels.get(1).unwrap_or(&"none".to_string()));
                if !resolved_labels.is_empty() {
                    println!("  Active filter:     {:?}", resolved_labels);
                }
                println!("  Synced tasks:      {}", synced_count);
                println!("  Assignee mappings: {}", config.mapping.assignees.len());
                match last_sync {
                    Some(t) => println!("  Last sync:         {}", t),
                    None => println!("  Last sync:         never"),
                }
            }
        }
    }

    Ok(())
}

pub fn unlink_cmd(quiet: bool) -> Result<()> {
    if !LinearConfig::exists() {
        if !quiet {
            println!("{} Linear is not configured.", INFO_PREFIX);
        }
        return Ok(());
    }

    if !quiet {
        print!("{} Remove Linear configuration and sync data? [y/N] ", WARNING_PREFIX);
        std::io::stdout().flush().ok();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).ok();
        if !input.trim().eq_ignore_ascii_case("y") {
            return Ok(());
        }
    }

    // Remove sync data from DB
    if let Ok(mut db) = ensure_initialized() {
        db.delete_all_linear_sync()?;
    }

    // Remove config
    LinearConfig::remove()?;

    if !quiet {
        println!("{} Linear integration removed.", SUCCESS_PREFIX);
    }

    Ok(())
}

fn print_stats(stats: &sync::SyncStats, quiet: bool) {
    if quiet {
        return;
    }

    println!("\n  Sync complete:");
    if stats.pushed_new > 0 {
        println!("    Pushed new:     {}", stats.pushed_new.to_string().green());
    }
    if stats.pushed_updated > 0 {
        println!("    Pushed updated: {}", stats.pushed_updated.to_string().blue());
    }
    if stats.pulled_new > 0 {
        println!("    Pulled new:     {}", stats.pulled_new.to_string().green());
    }
    if stats.pulled_updated > 0 {
        println!("    Pulled updated: {}", stats.pulled_updated.to_string().cyan());
    }
    if stats.conflicts > 0 {
        println!("    Conflicts:      {}", stats.conflicts.to_string().yellow());
    }
    if !stats.errors.is_empty() {
        println!("    Errors:");
        for err in &stats.errors {
            println!("      {} {}", ERROR_PREFIX, err);
        }
    }
    if stats.pushed_new + stats.pushed_updated + stats.pulled_new + stats.pulled_updated == 0 {
        println!("    Everything is up to date.");
    }
}
