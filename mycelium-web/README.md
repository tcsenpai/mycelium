# mycelium-web

A web frontend for [Mycelium](https://github.com/tcsenpai/mycelium) — the same
UI as the MycUI desktop app (Kanban / DAG / Slate boards, tasks, epics,
assignees, dependencies, follow-ups, dashboard), served over HTTP.

## Architecture

```
browser ──HTTP──> Bun+Hono server ──spawn──> myc CLI ──> .mycelium/ (SQLite)
                        │
                        └── serves the built React app (static)
```

- **`server/`** — Bun + Hono. Every `/api/*` route shells out to the `myc`
  CLI with `--format json` through one choke point (`myc.ts`). Writes are
  serialized through a process-wide chain so concurrent mutations can't race
  the SQLite WAL. Reads run unserialized.
- **`web/`** — the MycUI React app (`App.tsx` + `index.css`) reused verbatim;
  only `lib/api.ts` is swapped from Tauri `invoke()` to `fetch()`, and the
  Tauri event API is aliased to a no-op shim.

One server instance serves one project (the directory in `MYC_PROJECT_DIR`).

## Run with Docker (recommended)

```bash
# point at a directory containing a .mycelium project (or an empty one to init)
MYC_PROJECT_HOST=/path/to/your/project docker compose up --build
# open http://localhost:8787
```

The image builds the `myc` CLI, builds the frontend, and runs the server.

## Run locally (dev)

Requires `bun` and the `myc` binary on `PATH`.

```bash
# terminal 1 — API server against a project dir
cd server
MYC_PROJECT_DIR=/path/to/project bun run dev   # :8787

# terminal 2 — vite dev server (proxies /api to :8787)
cd web
bun install
bun run dev                                     # :5173
```

Production single-process:

```bash
cd web && bun install && bun run build          # -> web/dist
cp -r web/dist server/public
cd server && MYC_PROJECT_DIR=/path/to/project bun run start
```

## Environment

| Var               | Default        | Meaning                                  |
|-------------------|----------------|------------------------------------------|
| `MYC_PROJECT_DIR` | `cwd`          | Project dir holding `.mycelium/`         |
| `MYC_BIN`         | `myc`          | Path to the myc binary                   |
| `PORT`            | `8787`         | HTTP port                                |
| `MYC_TIMEOUT_MS`  | `15000`        | Per-CLI-call timeout                     |

## Known limitations

- **Removing a dependency** returns `501` — the `myc` CLI has no command to
  remove a `blocks` link (only add). Adding deps works.
- **No live push** — the UI polls (react-query) for freshness; there is no
  websocket/file-watcher like the desktop app's `database-changed` event.
- **Single project per server** — multi-folder switching is desktop-only.
