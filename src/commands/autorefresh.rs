//! Post-update auto-refresh of AGENTS.md and the Stop hook.
//!
//! The first time `myc` runs in a given repo after the binary was upgraded, it
//! ensures the project's generated artifacts match the new version: the
//! AGENTS.md mycelium block (checked by its embedded `v=N` marker) and the
//! installed Stop-hook script (checked by byte comparison against the embedded
//! copy). A global registry at `~/.mycelium/refresh-state.json` records, per
//! repo path, the myc version last reconciled there — so once a repo is up to
//! date for the running version the check short-circuits to a single file read
//! and returns. This is the cheap "flag" gate: no per-command cost after the
//! first run following an update.
//!
//! AGENTS.md refresh only ever rewrites the block *between* the markers, via
//! the existing `prime-agents` path, leaving user-authored text untouched. The
//! hook is refreshed only for scopes where it is already installed — we never
//! install a hook the user did not opt into.

use crate::cli::Commands;
use crate::commands::hooks::{self, Scope};
use std::path::PathBuf;

const MYC_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Global registry path: `~/.mycelium/refresh-state.json`. None if HOME unset.
fn registry_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| {
        PathBuf::from(h)
            .join(".mycelium")
            .join("refresh-state.json")
    })
}

/// Absolute path of the current project root (parent of the resolved
/// `.mycelium/`), used as the registry key. None outside a project.
fn project_key() -> Option<String> {
    let dir = crate::commands::get_mycelium_dir();
    let root = dir.parent()?;
    // Canonicalize so the same repo reached via different relative cwds keys
    // to one entry; fall back to the raw path if it can't be canonicalized.
    let abs = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    Some(abs.to_string_lossy().into_owned())
}

/// Read the version last reconciled for `key`, if the registry records one.
fn recorded_version(key: &str) -> Option<String> {
    let path = registry_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    json.get(key)?
        .get("myc_version")?
        .as_str()
        .map(str::to_owned)
}

/// Record that `key` is now reconciled for the running version. Best-effort:
/// a write failure just means the check re-runs next invocation, never an error.
fn record_version(key: &str) {
    let Some(path) = registry_path() else { return };
    let mut json: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = json.as_object_mut() {
        obj.insert(
            key.to_owned(),
            serde_json::json!({ "myc_version": MYC_VERSION }),
        );
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(&json) {
        let _ = std::fs::write(&path, s);
    }
}

/// Commands that must NOT trigger the check: they either manage the artifacts
/// themselves (init/hooks/prime-agents) or are trivial lookups where a refresh
/// notice would be noise and a project may not even exist.
fn is_exempt(cmd: &Commands) -> bool {
    matches!(
        cmd,
        Commands::Init { .. }
            | Commands::PrimeAgents { .. }
            | Commands::Hooks(_)
            | Commands::Doctor { .. }
            | Commands::Version
    )
}

/// Entry point, called once at startup. Self-gating and best-effort: any error
/// path silently returns so a refresh hiccup never blocks the real command.
pub fn maybe_refresh(cmd: &Commands) {
    if is_exempt(cmd) {
        return;
    }
    // Only inside an initialized project.
    if !crate::commands::get_db_path().exists() {
        return;
    }
    let Some(key) = project_key() else { return };

    // Cheap flag: already reconciled for this version -> nothing to do.
    if recorded_version(&key).as_deref() == Some(MYC_VERSION) {
        return;
    }

    // Full check (runs at most once per repo per version bump).
    let agents_stale = crate::commands::init::is_agents_block_stale();
    let stale_scopes: Vec<Scope> = hooks::ALL_SCOPES
        .into_iter()
        .filter(|&s| hooks::is_installed(s) && hooks::is_stale(s))
        .collect();

    if agents_stale {
        // Block-only rewrite; preserves user text. force=false is enough since
        // we already know the embedded version differs.
        if crate::commands::init::execute_prime_agents(false, None).is_ok() {
            eprintln!(
                "{} myc {}: AGENTS.md mycelium block refreshed",
                crate::INFO_PREFIX,
                MYC_VERSION
            );
        }
    }

    for scope in stale_scopes {
        if hooks::install(scope).is_ok() {
            eprintln!(
                "{} myc {}: {} Stop hook updated",
                crate::INFO_PREFIX,
                MYC_VERSION,
                scope_label(scope)
            );
        }
    }

    record_version(&key);
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Local => "local",
        Scope::Global => "global",
    }
}
