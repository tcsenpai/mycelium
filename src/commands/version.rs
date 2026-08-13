//! `myc version` — report the myc binary and the linked mycelium-core versions.
//!
//! `myc --version` (clap's built-in) only shows the binary. This surfaces both
//! so a core/CLI mismatch is visible at a glance — the core version comes from
//! `mycelium_core::VERSION` (what's actually linked), not the Cargo.toml dep
//! string, so it stays truthful even if that line drifts.

use crate::cli::OutputFormat;
use crate::commands::INFO_PREFIX;
use crate::error::Result;
use colored::Colorize;

const MYC_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn execute(format: &OutputFormat, quiet: bool) -> Result<()> {
    if quiet {
        return Ok(());
    }
    match format {
        OutputFormat::Json => {
            let out = serde_json::json!({
                "myc": MYC_VERSION,
                "core": mycelium_core::VERSION,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Table => {
            println!("{} myc  {}", INFO_PREFIX.blue(), MYC_VERSION.bold());
            println!(
                "{} core {}",
                INFO_PREFIX.blue(),
                mycelium_core::VERSION.bold()
            );
        }
    }
    Ok(())
}
