# mycelium-core extraction — plan

## Goal
CLI `myc` is the single source of truth. Extract `mycelium-core` (db + models +
migrations + error) from the CLI; both `myc` and MycUI consume it. MycUI drops
its parallel `db.rs`/`models.rs`. One schema, one Database API, no drift.

## Research findings (two agents, verified in code)

### CLI DB layer (src/db, src/models, src/error)
- `Database` wraps one `rusqlite::Connection`. ~60 pub methods across
  epic/task/assignee/deps/notes/external_ref/summary/linear/followup.
- `Task` is a bare row (no blocked_by/blocks). Denormalized data lives in
  `TaskWithRelations { task, epic_title?, assignee_name?, blocked_by, blocks,
  external_refs }` — which is NOT serde.
- Error: `MyceliumError` (thiserror enum, rich variants, `From<rusqlite>`,
  `From<serde_json>`). **NOT Serialize.** Two CLI-only fns (`handle_error`,
  `handle_usage_error`) reference `crate::ERROR_PREFIX` + `process::exit` — the
  ONLY CLI coupling in the db/models/error files.
- Migrations: self-contained, only deps `crate::error::Result` + rusqlite +
  chrono. Has the cross-branch `ensure_schema` net + busy_timeout(5s).
- Core deps needed: rusqlite(bundled,chrono), serde, serde_json, chrono(serde),
  thiserror. NOT needed: clap, colored, comfy-table, reqwest, sha2, etc.

### MycUI DB layer (mycui/src-tauri/src)
- Own `Database`, `Result<_, rusqlite::Error>` everywhere (domain errors abused
  into rusqlite variants). Tauri commands do `.map_err(|e| e.to_string())`.
- `Task` has blocked_by/blocks: Vec<i64> INLINE + epic_title/assignee_name
  denormalized, all serde snake_case → **the React frontend depends on this
  shape**.
- Method names differ: MycUI `get_tasks/get_epics`, CLI `list_tasks/list_epics`.
- MycUI-unique: `get_dashboard_stats` (10 metrics). CLI has `get_summary`
  (different shape).
- MycUI schema = SUBSET (epics/assignees/tasks/dependencies/followups only).
  Ignores linear_sync/task_notes/epic_notes/external_refs/v5 columns — but must
  tolerate them existing (CLI migrations create them).

## The real cost (why this isn't "just import")
The two `Database` APIs are NOT drop-in compatible. Adopting CLI-as-truth means:
1. MycUI's Rust backend rewires onto core's API (rename call sites, adapt Task
   shape).
2. **The React/TS frontend must change**: the serde shape MycUI sends to the
   frontend changes (bare Task + separate relations vs inline Vec), so
   `mycui/src/lib/types.ts` + `api.ts` + any field access in App.tsx need review.
3. Decide serde enum representation: today `InProgress`→`"inprogress"` on the
   wire, but TS expects `"in_progress"`. Fix with explicit `#[serde(rename)]` in
   core so DB (Display) and wire (serde) agree.

## Design decisions
- **DTO layer, not raw model reuse.** Core exposes the CLI models + a
  serde-friendly `TaskDto` (bare Task + blocked_by/blocks/epic_title/
  assignee_name) so MycUI keeps its inline shape without forcing blocked_by onto
  the CLI `Task`. Core provides a `get_tasks_with_relations`-style method that
  returns the populated DTO (CLI already has `get_dependencies_for_tasks` batch
  helper — reuse it, no N+1).
- **Error: add `impl Serialize for MyceliumError`** (serialize as
  `{code, message}`) so Tauri commands can return it. Keep the enum in core;
  move `handle_error`/`handle_usage_error` to the CLI binary.
- **Serde enums aligned to DB**: `#[serde(rename = "in_progress")]` etc. so wire
  == DB string. Update TS if any value changes (should end up MORE correct).
- **`get_dashboard_stats` moves into core** (it's pure SQL aggregation, useful
  to both; CLI can expose it too or ignore it).
- **Workspace**: root becomes `[workspace]` with members `.` (myc),
  `mycelium-core`, `mycui/src-tauri`. myc + mycui depend on core via path.
  Publish impact tracked in follow-up #10.

## Phases

### Phase 0: Safety net
- [ ] Snapshot current behavior: `cargo test` green, MycUI `tauri build` green,
      note current frontend Task JSON shape (capture a sample get_tasks payload).
- [ ] Branch: `feature/mycelium-core`.

### Phase 1: Create the crate (no behavior change to CLI)
- [ ] `mycelium-core/` with Cargo.toml (minimal deps).
- [ ] Move `src/error` (minus the 2 CLI fns), `src/models/*`, `src/db/mod.rs`,
      `src/db/migrations.rs` into core. Re-export `Result`, `MyceliumError`.
- [ ] Add `impl Serialize for MyceliumError`.
- [ ] Add serde `rename` on enums to match DB strings.
- [ ] Add `TaskDto` + a method returning populated DTOs.
- [ ] Root Cargo.toml → workspace; `myc` depends on `mycelium-core`.
- [ ] Keep the 2 error helper fns in the CLI binary (src/), referencing core's
      enum.
- [ ] `cargo test` green (CLI unchanged behavior). This is a checkpoint —
      CLI fully works on core before touching MycUI.

### Phase 2: Rewire MycUI backend
- [ ] `mycui/src-tauri/Cargo.toml` depends on `mycelium-core`.
- [ ] Delete `mycui/src-tauri/src/db.rs` + `models.rs`.
- [ ] Tauri commands call core methods (rename get_→list_ where needed; use DTOs
      for get_tasks/get_task; use core get_dashboard_stats).
- [ ] Commands return `Result<T, MyceliumError>` (now Serialize) or map to
      string — pick one, consistently.
- [ ] `cargo check` + `cargo build` the tauri crate green.

### Phase 3: Align the frontend
- [ ] Diff the new get_tasks JSON vs the captured Phase-0 sample.
- [ ] Update `mycui/src/lib/types.ts` (Task/Epic/etc.) + `api.ts` to match core's
      serde shape (esp. status enum values, blocked_by/blocks location).
- [ ] Fix any field access in App.tsx that shifted.
- [ ] `tsc && vite build` green.

### Phase 4: Verify end-to-end
- [ ] `cargo test` (workspace) green — incl. the schema-equality test, which
      becomes trivially true (one schema) or is replaced by a core unit test.
- [ ] `tauri build` produces a working bundle.
- [ ] Manual smoke: open a real .mycelium project in MycUI, tasks/epics/followups
      render, status change + edit modal work, DAG shows blocked_by.
- [ ] Delete the now-redundant interim schema-equality integration test (or keep
      as a guard — cheap).

### Phase 5: Cleanup + follow-ups
- [ ] Update follow-up #10 (publish strategy) with the concrete workspace layout.
- [ ] Close follow-up #1.
- [ ] Update README architecture section (one shared core crate).

## Risks / edge cases
- **Frontend breakage** is the highest risk (Phase 3). Mitigated by capturing the
  Phase-0 JSON sample and diffing.
- **Publish**: workspace + path-dep changes `cargo publish` for myc (follow-up
  #10). Does NOT block the extraction; only the next crates.io release.
- **rusqlite `uuid` feature**: agent flagged it may be unused — verify, drop from
  core if so.
- **Unread models** (`assignee.rs`, `dependency.rs`, note/ref models): verify
  clean (no CLI coupling) before moving.
- **Enum serde rename**: changing wire values could break the frontend if TS
  hardcodes old values — Phase 3 catches this.

## Rollback
All on `feature/mycelium-core` branch. If Phase 3 frontend churn is too costly,
the branch can be abandoned; main keeps the two-impl + schema-equality-test
status quo (already shipped, safe).
