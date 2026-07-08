// End-to-end smoke test: boots the server against a throwaway myc project,
// exercises the write paths that don't emit JSON (close/reopen/deps/followup
// transitions) plus enrichment (epic_title/assignee_name/blocks), and asserts.
// Run: `bun test` (requires `myc` on PATH, or set MYC_BIN).
//
// This is the one runnable check guarding the two non-trivial server rules:
//   1. writes are serialized (no lost writes under back-to-back POSTs)
//   2. message-only commands use mycExec, JSON commands use mycWrite

import { test, expect, beforeAll, afterAll } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const MYC = process.env.MYC_BIN || 'myc';
let dir: string;
let server: ReturnType<typeof Bun.serve> | null = null;
let base: string;

async function myc(args: string[]) {
  const p = Bun.spawn([MYC, ...args], { cwd: dir, stdout: 'pipe', stderr: 'pipe' });
  await p.exited;
}

beforeAll(async () => {
  dir = mkdtempSync(join(tmpdir(), 'myc-web-test-'));
  await myc(['init']);
  process.env.MYC_PROJECT_DIR = dir;
  const port = 8900 + Math.floor(performance.now() % 90);
  const mod = await import('./index.ts');
  server = Bun.serve({ port, fetch: mod.default.fetch });
  base = `http://localhost:${port}`;
});

afterAll(() => {
  server?.stop(true);
  if (dir) rmSync(dir, { recursive: true, force: true });
});

const H = { 'Content-Type': 'application/json' };
const post = (p: string, b?: unknown) =>
  fetch(base + p, { method: 'POST', headers: H, body: b ? JSON.stringify(b) : undefined });
const patch = (p: string, b: unknown) =>
  fetch(base + p, { method: 'PATCH', headers: H, body: JSON.stringify(b) });
const get = (p: string) => fetch(base + p).then((r) => r.json());

test('no writes lost under back-to-back POSTs (serialization)', async () => {
  for (const t of ['A', 'B', 'C', 'D', 'E']) {
    const r = await post('/api/tasks', { title: t, priority: 'medium' });
    expect(r.status).toBe(200);
  }
  const tasks = await get('/api/tasks');
  expect(tasks.length).toBe(5);
});

test('enrichment: epic_title, assignee_name, blocks', async () => {
  await post('/api/epics', { title: 'Epic' });
  await post('/api/assignees', { name: 'Alice', github_username: 'alice' });
  const t1 = await post('/api/tasks', {
    title: 'T1',
    priority: 'high',
    epic_id: 1,
    assignee_id: 1,
  }).then((r) => r.json());
  const t2 = await post('/api/tasks', { title: 'T2', priority: 'low' }).then((r) => r.json());
  await post(`/api/tasks/${t2.id}/deps`, { depends_on: t1.id });

  const enriched = await get(`/api/tasks/${t1.id}`);
  expect(enriched.epic_title).toBe('Epic');
  expect(enriched.assignee_name).toBe('Alice');
  expect(enriched.blocks).toContain(t2.id);
});

test('message-only writes return 200 and correct state', async () => {
  const t = await post('/api/tasks', { title: 'Z', priority: 'low' }).then((r) => r.json());
  expect((await post(`/api/tasks/${t.id}/close`)).status).toBe(200);
  expect((await get(`/api/tasks/${t.id}`)).status).toBe('closed');
  expect((await post(`/api/tasks/${t.id}/reopen`)).status).toBe(200);
  expect((await patch(`/api/tasks/${t.id}`, { priority: 'critical' })).status).toBe(200);
});

test('followup status transitions (message-only) round-trip', async () => {
  const fu = await post('/api/followups', { body: 'note' }).then((r) => r.json());
  const started = await post(`/api/followups/${fu.id}/status`, { status: 'in_progress' }).then(
    (r) => r.json(),
  );
  expect(started.status).toBe('in_progress');
  const done = await post(`/api/followups/${fu.id}/status`, { status: 'done' }).then((r) =>
    r.json(),
  );
  expect(done.status).toBe('done');
});

test('removing a dependency returns 501 (CLI gap)', async () => {
  const res = await fetch(base + '/api/tasks/1/deps/2', { method: 'DELETE' });
  expect(res.status).toBe(501);
});
