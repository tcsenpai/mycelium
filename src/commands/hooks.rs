//! Install / uninstall the mycelium Claude Code Stop hook.
//!
//! The hook script (`hooks/myc-followup-stop.sh`) is embedded at compile time,
//! so it travels with the `myc` binary — no separate files needed for
//! `cargo install` users. `myc init` installs it project-locally; `myc hooks`
//! manages it explicitly (local or global). Settings merge is idempotent and
//! never touches unrelated hooks.

use crate::commands::{INFO_PREFIX, SUCCESS_PREFIX};
use crate::error::{MyceliumError, Result};
use colored::Colorize;
use serde_json::{json, Value};
use std::path::PathBuf;

/// The Stop hook script, embedded from the single source of truth in `hooks/`.
const HOOK_SCRIPT: &str = include_str!("../../hooks/myc-followup-stop.sh");
const HOOK_NAME: &str = "myc-followup-stop.sh";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `.claude/` in the current project (committable, follows the repo).
    Local,
    /// `~/.claude/` (applies to every mycelium project on this machine).
    Global,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Scope::Local => "local",
            Scope::Global => "global",
        }
    }

    /// `.claude` directory for this scope.
    fn claude_dir(self) -> Result<PathBuf> {
        match self {
            Scope::Local => Ok(PathBuf::from(".claude")),
            Scope::Global => {
                let home = std::env::var_os("HOME").ok_or_else(|| {
                    MyceliumError::Config("HOME is not set; cannot resolve ~/.claude".into())
                })?;
                Ok(PathBuf::from(home).join(".claude"))
            }
        }
    }

    /// The `command` string stored in settings.json. Local uses a relative
    /// path (portable when committed); global uses `$HOME` so it stays portable
    /// across machines and matches install-hook.sh.
    fn command_string(self) -> String {
        match self {
            Scope::Local => format!(".claude/hooks/{HOOK_NAME}"),
            Scope::Global => format!("$HOME/.claude/hooks/{HOOK_NAME}"),
        }
    }
}

pub fn install(scope: Scope) -> Result<()> {
    let claude_dir = scope.claude_dir()?;
    let hooks_dir = claude_dir.join("hooks");
    let dest = hooks_dir.join(HOOK_NAME);
    let settings = claude_dir.join("settings.json");
    let cmd = scope.command_string();

    // 1. Write the script (always a fresh copy) and mark it executable.
    std::fs::create_dir_all(&hooks_dir)?;
    std::fs::write(&dest, HOOK_SCRIPT)?;
    set_executable(&dest)?;

    // 2. Wire into hooks.Stop idempotently.
    let added = wire_settings(&settings, &cmd)?;

    println!(
        "{} Installed follow-up hook ({}, {})",
        SUCCESS_PREFIX.green(),
        dest.display(),
        scope.label()
    );
    if !added {
        println!(
            "{} settings.json already wired (no change)",
            INFO_PREFIX.blue()
        );
    }
    Ok(())
}

pub fn uninstall(scope: Scope) -> Result<()> {
    let claude_dir = scope.claude_dir()?;
    let dest = claude_dir.join("hooks").join(HOOK_NAME);
    let settings = claude_dir.join("settings.json");
    let cmd = scope.command_string();

    let removed = unwire_settings(&settings, &cmd)?;
    if dest.exists() {
        std::fs::remove_file(&dest)?;
    }

    println!(
        "{} Removed follow-up hook ({})",
        SUCCESS_PREFIX.green(),
        scope.label()
    );
    if !removed {
        println!("{} settings.json had no matching entry", INFO_PREFIX.blue());
    }
    Ok(())
}

pub fn status() -> Result<()> {
    for scope in [Scope::Local, Scope::Global] {
        let claude_dir = scope.claude_dir()?;
        let dest = claude_dir.join("hooks").join(HOOK_NAME);
        let settings = claude_dir.join("settings.json");
        let cmd = scope.command_string();

        let script_present = dest.exists();
        let wired = settings_contains(&settings, &cmd)?;
        let mark = if script_present && wired {
            "✅ installed"
        } else if script_present || wired {
            "⚠️  partial"
        } else {
            "— not installed"
        };
        println!("  {:<7} {}", scope.label(), mark);
    }
    Ok(())
}

// --- settings.json manipulation (idempotent, serde_json based) ---

/// Append the Stop-hook entry if absent. Returns true if it was added.
fn wire_settings(settings: &std::path::Path, cmd: &str) -> Result<bool> {
    let mut root = read_settings(settings)?;

    if stop_has_command(&root, cmd) {
        return Ok(false);
    }

    let stop = stop_array_mut(&mut root);
    stop.push(json!({ "hooks": [ { "type": "command", "command": cmd } ] }));

    write_settings(settings, &root)?;
    Ok(true)
}

/// Drop any Stop entry whose hooks reference `cmd`. Returns true if any removed.
fn unwire_settings(settings: &std::path::Path, cmd: &str) -> Result<bool> {
    if !settings.exists() {
        return Ok(false);
    }
    let mut root = read_settings(settings)?;

    let stop = match root
        .get_mut("hooks")
        .and_then(|h| h.get_mut("Stop"))
        .and_then(|s| s.as_array_mut())
    {
        Some(s) => s,
        None => return Ok(false),
    };

    let before = stop.len();
    stop.retain(|entry| !entry_has_command(entry, cmd));
    let removed = stop.len() != before;

    if removed {
        write_settings(settings, &root)?;
    }
    Ok(removed)
}

fn settings_contains(settings: &std::path::Path, cmd: &str) -> Result<bool> {
    if !settings.exists() {
        return Ok(false);
    }
    let root = read_settings(settings)?;
    Ok(stop_has_command(&root, cmd))
}

fn read_settings(settings: &std::path::Path) -> Result<Value> {
    if !settings.exists() {
        return Ok(json!({}));
    }
    let text = std::fs::read_to_string(settings)?;
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text).map_err(|e| {
        MyceliumError::Config(format!(
            "{} is not valid JSON ({e}); refusing to overwrite. Fix it by hand.",
            settings.display()
        ))
    })
}

fn write_settings(settings: &std::path::Path, root: &Value) -> Result<()> {
    let dir = settings
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    std::fs::create_dir_all(dir)?;
    let body = serde_json::to_string_pretty(root)?;
    // Atomic-ish: write to a temp file in the same dir, then rename.
    let tmp = settings.with_extension("json.tmp");
    std::fs::write(&tmp, body.as_bytes())?;
    std::fs::rename(&tmp, settings)?;
    Ok(())
}

/// Ensure `.hooks.Stop` exists as an array and return a mutable ref to it.
fn stop_array_mut(root: &mut Value) -> &mut Vec<Value> {
    if !root.is_object() {
        *root = json!({});
    }
    let obj = root.as_object_mut().unwrap();
    let hooks = obj.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks_obj = hooks.as_object_mut().unwrap();
    let stop = hooks_obj.entry("Stop").or_insert_with(|| json!([]));
    if !stop.is_array() {
        *stop = json!([]);
    }
    stop.as_array_mut().unwrap()
}

fn stop_has_command(root: &Value, cmd: &str) -> bool {
    root.get("hooks")
        .and_then(|h| h.get("Stop"))
        .and_then(|s| s.as_array())
        .map(|arr| arr.iter().any(|e| entry_has_command(e, cmd)))
        .unwrap_or(false)
}

fn entry_has_command(entry: &Value, cmd: &str) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks
                .iter()
                .any(|h| h.get("command").and_then(|c| c.as_str()) == Some(cmd))
        })
        .unwrap_or(false)
}

#[cfg(unix)]
fn set_executable(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write(path: &Path, v: &Value) {
        std::fs::write(path, serde_json::to_string_pretty(v).unwrap()).unwrap();
    }

    fn tmpfile(name: &str) -> PathBuf {
        // Unique-ish per test name; cleaned by the OS temp dir eventually.
        let mut p = std::env::temp_dir();
        p.push(format!("myc-hooks-test-{name}.json"));
        let _ = std::fs::remove_file(&p);
        p
    }

    #[test]
    fn merge_into_empty_creates_one_entry() {
        let f = tmpfile("empty");
        let added = wire_settings(&f, ".claude/hooks/x.sh").unwrap();
        assert!(added);
        let root = read_settings(&f).unwrap();
        let stop = root["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert!(stop_has_command(&root, ".claude/hooks/x.sh"));
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn merge_preserves_unrelated_hooks() {
        let f = tmpfile("unrelated");
        write(
            &f,
            &json!({
                "hooks": { "Stop": [
                    { "hooks": [ { "type": "command", "command": "$HOME/other.sh" } ] }
                ] },
                "model": "opus"
            }),
        );
        wire_settings(&f, ".claude/hooks/x.sh").unwrap();
        let root = read_settings(&f).unwrap();
        assert_eq!(root["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert!(stop_has_command(&root, "$HOME/other.sh"));
        assert!(stop_has_command(&root, ".claude/hooks/x.sh"));
        assert_eq!(root["model"], "opus"); // untouched
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn merge_is_idempotent() {
        let f = tmpfile("idem");
        assert!(wire_settings(&f, ".claude/hooks/x.sh").unwrap());
        assert!(!wire_settings(&f, ".claude/hooks/x.sh").unwrap()); // no dup
        let root = read_settings(&f).unwrap();
        assert_eq!(root["hooks"]["Stop"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn uninstall_removes_only_ours() {
        let f = tmpfile("uninstall");
        write(
            &f,
            &json!({ "hooks": { "Stop": [
                { "hooks": [ { "type": "command", "command": "$HOME/other.sh" } ] },
                { "hooks": [ { "type": "command", "command": ".claude/hooks/x.sh" } ] }
            ] } }),
        );
        let removed = unwire_settings(&f, ".claude/hooks/x.sh").unwrap();
        assert!(removed);
        let root = read_settings(&f).unwrap();
        let stop = root["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert!(stop_has_command(&root, "$HOME/other.sh"));
        assert!(!stop_has_command(&root, ".claude/hooks/x.sh"));
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn malformed_settings_errors_not_clobbers() {
        let f = tmpfile("malformed");
        std::fs::write(&f, "{ not json").unwrap();
        assert!(wire_settings(&f, ".claude/hooks/x.sh").is_err());
        // File left intact.
        assert_eq!(std::fs::read_to_string(&f).unwrap(), "{ not json");
        let _ = std::fs::remove_file(&f);
    }
}
