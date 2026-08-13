//! mycelium-core: shared database, models, migrations, and error types.
//!
//! This crate is the single source of truth for the Mycelium data layer,
//! consumed by both the `myc` CLI and the MycUI desktop app. It contains no
//! CLI- or GUI-specific code (no clap, colored, println!, or Tauri).

/// This crate's own version, for `myc version` to report the actual linked
/// core (truthful even if Cargo.toml's dep line drifts from what's built).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod db;
pub mod error;
pub mod id;
pub mod models;

pub use db::Database;
pub use error::{MyceliumError, Result};
pub use id::{format_id, parse_id, IdKind};
