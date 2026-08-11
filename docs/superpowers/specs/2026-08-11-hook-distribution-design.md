# Hook distribution via `myc init` + `myc hooks`

**Date:** 2026-08-11
**Status:** approved

## Problem

The mycelium follow-up Stop hook lives in `hooks/` and is installed only by
`hooks/install-hook.sh` into `~/.claude`. `cargo install mycelium-manager`
ships only the `myc` binary — the `hooks/` dir is `exclude`d from the crate, so
crates.io users get no hook and no installer. The end-of-task follow-up check
therefore silently doesn't exist for them.

Additionally, the installed hook counted `open + in_progress`, contradicting the
documented rule ("count only `open`"). Fixed separately (see Non-goals).

## Goals

- `cargo install`-able distribution: the hook script travels with the binary.
- `myc init` auto-installs the hook into the **project-local** `.claude/`
  (committable, follows the repo, whole team gets the check). Opt-out via
  `--no-hooks`.
- Explicit management: `myc hooks install|uninstall [--global|--local]`.
- No double-firing when both a global (`~/.claude`) and a local hook are wired.

## Non-goals

- Changing the hook's follow-up logic beyond the already-shipped `active→open`
  fix (that fix is done and installed).
- Auto-updating an already-installed hook script on version bump (install always
  overwrites the script with a fresh copy; that's enough).
- Removing `install-hook.sh` (kept for global bash-only installs).

## Architecture

Three pieces.

### 1. Single source of truth for the script

`hooks/myc-followup-stop.sh` stays the ONE script file. The Rust module embeds
it at compile time:

```rust
const HOOK_SCRIPT: &str = include_str!("../../hooks/myc-followup-stop.sh");
const HOOK_NAME: &str = "myc-followup-stop.sh";
```

`Cargo.toml` `exclude` loses the `hooks/` entry (currently `hooks/` is not
explicitly excluded but is not shipped either — add it to the package by NOT
excluding and confirming `cargo package --list` includes it). `install-hook.sh`
keeps using the same file → no drift.

### 2. `src/commands/hooks.rs`

Shared install/uninstall logic, `serde_json`-based (no `jq` dependency):

```rust
pub enum Scope { Local, Global }

pub fn install(scope: Scope) -> Result<()>
pub fn uninstall(scope: Scope) -> Result<()>
pub fn status() -> Result<()>   // report where the hook is wired (local/global)
```

- **Paths.** Local → `.claude/hooks/myc-followup-stop.sh` + `.claude/settings.json`
  in cwd. Global → `~/.claude/hooks/...` + `~/.claude/settings.json`.
- **Command string stored in settings.** Local: `.claude/hooks/myc-followup-stop.sh`
  (relative — portable across machines when committed). Global:
  `$HOME/.claude/hooks/myc-followup-stop.sh` (matches `install-hook.sh`).
- **Script write.** `mkdir -p <hooks dir>`, write `HOOK_SCRIPT`, `chmod 0755`.
  Always overwrites (fresh copy).
- **Settings merge (idempotent).** Read file or `{}` → parse JSON → ensure
  `.hooks.Stop` is an array → append `{ "hooks": [ { "type": "command",
  "command": <CMD> } ] }` ONLY if no existing `Stop[].hooks[].command == CMD`.
  Write atomically (tmp file in same dir + rename). Other hooks (graft, etc.)
  are never touched. Preserves key order best-effort via `serde_json` (object
  order not guaranteed, acceptable — it's a settings file).
- **Uninstall.** Remove Stop entries whose `hooks[].command == CMD`; delete the
  script file. Leave settings.json otherwise intact.

### 3. Self-dedup in the hook script

Both a global and a local hook can be wired simultaneously → the Stop check
would fire twice. The single script self-gates so only the FIRST invocation per
stop proceeds:

- The Stop payload (stdin JSON) carries `session_id` (verified: other hooks read
  it). Build a per-stop marker: `"$TMPDIR"/.myc-fu-stop-<session_id-or-hash>`.
- On entry, after the existing `stop_hook_active` guard: if the marker exists and
  is younger than a short TTL (e.g. 10s), `exit 0`. Otherwise `touch` it and
  proceed. Falls back to a hash of `transcript_path`, then to a cwd-based
  lockfile in `.mycelium/` if neither field is present.
- Marker is short-lived; stale markers are harmless (next real stop refreshes).

## CLI surface

- `Commands::Hooks(HooksCommands)` with `install { global: bool }`,
  `uninstall { global: bool }`, `status`. `--global` flag; absence = local.
- `myc init` calls `hooks::install(Scope::Local)` after AGENTS.md, guarded by a
  new `--no-hooks` flag on `Init`. Idempotent (re-init = no duplicate).
- `init` prints a one-line note: `✓ Installed follow-up hook (.claude/, local)`
  or `(skipped: --no-hooks)`.

## Error handling

- Missing `~` / HOME for global scope → clear error.
- Malformed existing settings.json → error out with the path, do NOT clobber.
- Write failures → propagate `MyceliumError`.
- `init` hook install failure is NON-fatal: warn, continue (init still succeeds).

## Testing

- `hooks.rs` unit tests (temp dirs):
  - merge into empty settings → one Stop entry.
  - merge into settings with an unrelated hook → both present, unrelated intact.
  - merge when already present → no duplicate (idempotent).
  - uninstall removes only our entry, leaves others.
- Self-dedup: bash `assert`-style check — two invocations with the same
  `session_id` → second is a no-op (checked via a sentinel side effect).
- Existing integration tests unaffected (init still writes AGENTS.md etc.).

## Rollout

- Core API unchanged → no core bump. Main crate is a feature (new command) →
  minor bump `0.3.1 → 0.4.0`. `hooks/` now shipped in the crate.
