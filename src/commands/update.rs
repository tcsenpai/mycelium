//! `myc update` — update the `myc` binary, then resync project artifacts.
//!
//! Flow:
//! 1. `cargo install mycelium-manager --force` (cargo is the official channel;
//!    it handles download + checksum + replacing the in-use binary — we do NOT
//!    reinvent a self-updater).
//! 2. Resync AGENTS.md + the follow-up hook using the NEWLY installed binary
//!    (re-invoked from PATH), so the artifacts match the new version rather than
//!    the old process still running here.
//!
//! If cargo is missing, the binary can't be updated; we still resync artifacts
//! with the current binary and tell the user to update myc by hand.

use crate::commands::{INFO_PREFIX, SUCCESS_PREFIX, WARNING_PREFIX};
use crate::error::Result;
use colored::Colorize;
use std::process::Command;

const CRATE_NAME: &str = "mycelium-manager";

pub fn execute() -> Result<()> {
    let has_cargo = which("cargo");

    if has_cargo {
        println!("{} Updating {} via cargo…", INFO_PREFIX.blue(), CRATE_NAME);
        let status = Command::new("cargo")
            .args(["install", CRATE_NAME, "--force"])
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("{} Binary updated", SUCCESS_PREFIX.green());
                // Resync with the NEW binary from PATH so artifacts match it.
                resync_via_new_binary();
                return Ok(());
            }
            Ok(s) => {
                println!(
                    "{} cargo install exited with {} — binary NOT updated. \
                     Syncing project artifacts with the current binary instead.",
                    WARNING_PREFIX.yellow(),
                    s
                );
            }
            Err(e) => {
                println!(
                    "{} could not run cargo ({e}) — syncing project artifacts \
                     with the current binary instead.",
                    WARNING_PREFIX.yellow()
                );
            }
        }
    } else {
        println!(
            "{} cargo not found — cannot update the binary. Update myc by hand \
             (e.g. your package manager or the release page), then rerun \
             `myc update`. Syncing project artifacts with the current binary now.",
            WARNING_PREFIX.yellow()
        );
    }

    // Fallback path: resync in-process with the current binary.
    resync_in_process();
    Ok(())
}

/// Resync AGENTS.md + hook by invoking the freshly installed `myc` from PATH.
/// A failure here is non-fatal — the binary update already succeeded.
fn resync_via_new_binary() {
    println!("{} Syncing project artifacts…", INFO_PREFIX.blue());

    run_new_myc(&["prime-agents", "--force"], "AGENTS.md");
    run_new_myc(&["hooks", "install"], "follow-up hook");
}

fn run_new_myc(args: &[&str], label: &str) {
    match Command::new("myc").args(args).status() {
        Ok(s) if s.success() => {}
        Ok(s) => println!(
            "{} `myc {}` exited with {} (run it manually to sync {label})",
            WARNING_PREFIX.yellow(),
            args.join(" "),
            s
        ),
        Err(e) => println!(
            "{} could not run `myc {}` ({e}) — sync {label} manually",
            WARNING_PREFIX.yellow(),
            args.join(" ")
        ),
    }
}

/// Resync using the current process's code (when the binary wasn't updated).
fn resync_in_process() {
    if let Err(e) = crate::commands::init::execute_prime_agents(true, None) {
        println!(
            "{} could not sync AGENTS.md: {e} (run `myc prime-agents --force`)",
            WARNING_PREFIX.yellow()
        );
    }
    if let Err(e) = crate::commands::hooks::install(crate::commands::hooks::Scope::Local) {
        println!(
            "{} could not sync hook: {e} (run `myc hooks install`)",
            WARNING_PREFIX.yellow()
        );
    }
    println!("{} Project artifacts synced", SUCCESS_PREFIX.green());
}

/// Is `cmd` on PATH? Uses `PATH` scan (no external `which` dependency).
fn which(cmd: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| {
        let p = dir.join(cmd);
        p.is_file() || p.with_extension("exe").is_file()
    })
}
