import { Hono } from 'hono';
import { serveStatic } from 'hono/bun';
import { mycRead, mycWrite, mycExec, MycError, projectDir } from './myc';
import { enrichTasks } from './enrich';

const app = new Hono();

// --- error mapping -----------------------------------------------------------
app.onError((err, c) => {
  if (err instanceof MycError) {
    const status = err.code === 0 ? 502 : 400;
    return c.json({ error: err.message }, status);
  }
  return c.json({ error: String((err as Error).message ?? err) }, 500);
});

const api = new Hono();

// helper: build `myc task update` style flag args, skipping undefined
function flags(pairs: Array<[string, unknown]>): string[] {
  const out: string[] = [];
  for (const [flag, val] of pairs) {
    if (val === undefined) continue;
    out.push(flag, String(val));
  }
  return out;
}

// --- project meta (folder-picker shims for the ported UI) --------------------
api.get('/path', (c) => c.json(projectDir()));
api.get('/recent-folders', (c) => c.json([]));
api.get('/current-db-path', (c) => c.json(projectDir()));

// --- dashboard ---------------------------------------------------------------
api.get('/summary', async (c) => c.json(await mycRead(['summary'])));

// --- tasks -------------------------------------------------------------------
api.get('/tasks', async (c) => {
  const q = c.req.query();
  // Always fetch every status: `myc task list` without --all hides both
  // closed AND in_progress tasks, but the board needs all columns. Status
  // filtering happens client-side (and via the ?status= query below).
  const bare = await mycRead<Array<{ id: number }>>(['task', 'list', '--all']);
  let tasks = await enrichTasks(bare.map((t) => t.id));

  // apply filters server-side (myc task list has limited filter flags)
  if (q.epic_id) tasks = tasks.filter((t) => t.epic_id === Number(q.epic_id));
  if (q.status) tasks = tasks.filter((t) => t.status === q.status);
  if (q.priority) tasks = tasks.filter((t) => t.priority === q.priority);
  if (q.assignee_id) tasks = tasks.filter((t) => t.assignee_id === Number(q.assignee_id));
  if (q.tag)
    tasks = tasks.filter((t) =>
      String(t.tags ?? '')
        .split(',')
        .map((s) => s.trim())
        .includes(q.tag),
    );
  if (q.blocked === 'true') tasks = tasks.filter((t) => t.blocked_by.length > 0);
  if (q.search) {
    const needle = q.search.toLowerCase();
    tasks = tasks.filter(
      (t) =>
        String(t.title).toLowerCase().includes(needle) ||
        String(t.description ?? '').toLowerCase().includes(needle),
    );
  }
  return c.json(tasks);
});

api.get('/tasks/:id', async (c) => {
  const id = Number(c.req.param('id'));
  const [enriched] = await enrichTasks([id]);
  return c.json(enriched ?? null);
});

api.post('/tasks', async (c) => {
  const b = await c.req.json();
  const args = [
    'task',
    'create',
    '--title',
    b.title,
    ...flags([
      ['--description', b.description],
      ['--priority', b.priority],
      ['--epic', b.epic_id],
      ['--assignee', b.assignee_id],
      ['--due', b.due_date],
      ['--tags', b.tags],
    ]),
  ];
  const created = await mycWrite<{ id: number }>(args);
  const [enriched] = await enrichTasks([created.id]);
  return c.json(enriched);
});

api.patch('/tasks/:id', async (c) => {
  const id = Number(c.req.param('id'));
  const u = await c.req.json();
  // map null-to-clear semantics: epic/assignee use 0, others use '-'
  const args = [
    'task',
    'update',
    String(id),
    ...flags([
      ['--title', u.title],
      ['--description', u.description],
      ['--status', u.status],
      ['--priority', u.priority],
      ['--epic', u.epic_id === null ? 0 : u.epic_id],
      ['--assignee', u.assignee_id === null ? 0 : u.assignee_id],
      ['--due', u.due_date === null ? '-' : u.due_date],
      ['--tags', u.tags === null ? '-' : u.tags],
    ]),
  ];
  await mycWrite(args);
  const [enriched] = await enrichTasks([id]);
  return c.json(enriched);
});

api.delete('/tasks/:id', async (c) => {
  await mycExec(['task', 'delete', c.req.param('id'), '--force']);
  return c.json({ ok: true });
});

api.post('/tasks/:id/close', async (c) => {
  const id = Number(c.req.param('id'));
  await mycExec(['task', 'close', String(id), '--force']);
  const [e] = await enrichTasks([id]);
  return c.json(e);
});

api.post('/tasks/:id/reopen', async (c) => {
  const id = Number(c.req.param('id'));
  await mycExec(['task', 'reopen', String(id)]);
  const [e] = await enrichTasks([id]);
  return c.json(e);
});

api.post('/tasks/:id/start', async (c) => {
  const id = Number(c.req.param('id'));
  await mycExec(['task', 'update', String(id), '--status', 'in_progress']);
  const [e] = await enrichTasks([id]);
  return c.json(e);
});

api.get('/search', async (c) => {
  const query = c.req.query('q') ?? '';
  const bare = await mycRead<Array<{ id: number }>>(['task', 'list', '--all']);
  const tasks = await enrichTasks(bare.map((t) => t.id));
  const needle = query.toLowerCase();
  return c.json(
    tasks.filter(
      (t) =>
        String(t.title).toLowerCase().includes(needle) ||
        String(t.description ?? '').toLowerCase().includes(needle),
    ),
  );
});

api.get('/tags', async (c) => {
  const bare = await mycRead<Array<{ tags?: string | null }>>(['task', 'list', '--all']);
  const set = new Set<string>();
  for (const t of bare) {
    for (const tag of (t.tags ?? '').split(',')) {
      const trimmed = tag.trim();
      if (trimmed) set.add(trimmed);
    }
  }
  return c.json([...set].sort((a, b) => a.localeCompare(b)));
});

// --- deps --------------------------------------------------------------------
api.post('/tasks/:id/deps', async (c) => {
  const id = c.req.param('id');
  const { depends_on } = await c.req.json();
  // "task <depends_on> blocks task <id>"
  await mycExec(['task', 'link', 'blocks', '--task', String(depends_on), String(id)]);
  return c.json({ ok: true });
});

api.delete('/tasks/:id/deps/:dependsOn', (c) =>
  // The myc CLI has no command to remove a `blocks` dependency
  // (`task unlink` only removes external refs). Surface the gap explicitly.
  c.json({ error: 'Removing dependencies is not supported by the myc CLI' }, 501),
);

// --- epics -------------------------------------------------------------------
api.get('/epics', async (c) => c.json(await mycRead(['epic', 'list'])));
api.get('/epics/:id', async (c) => c.json(await mycRead(['epic', 'show', c.req.param('id')])));

api.post('/epics', async (c) => {
  const b = await c.req.json();
  return c.json(
    await mycWrite(['epic', 'create', '--title', b.title, ...flags([['--description', b.description]])]),
  );
});

api.patch('/epics/:id', async (c) => {
  const u = await c.req.json();
  const args = [
    'epic',
    'update',
    c.req.param('id'),
    ...flags([
      ['--title', u.title],
      ['--description', u.description],
      ['--status', u.status],
    ]),
  ];
  return c.json(await mycWrite(args));
});

api.delete('/epics/:id', async (c) => {
  await mycExec(['epic', 'delete', c.req.param('id'), '--force']);
  return c.json({ ok: true });
});

// --- assignees ---------------------------------------------------------------
api.get('/assignees', async (c) => c.json(await mycRead(['assignee', 'list'])));

api.post('/assignees', async (c) => {
  const b = await c.req.json();
  const args = [
    'assignee',
    'create',
    '--name',
    b.name,
    ...flags([
      ['--email', b.email],
      ['--github', b.github_username],
    ]),
  ];
  return c.json(await mycWrite(args));
});

// --- followups ---------------------------------------------------------------
api.get('/followups', async (c) => {
  const includeClosed = c.req.query('includeClosed') === 'true';
  const args = ['followup', 'list'];
  if (!includeClosed) args.push('-o');
  return c.json(await mycRead(args));
});

api.get('/followups/count', async (c) => c.json(await mycRead(['followup', 'count'])));
api.get('/followups/:id', async (c) => c.json(await mycRead(['followup', 'show', c.req.param('id')])));

api.post('/followups', async (c) => {
  const b = await c.req.json();
  const args = ['followup', 'add', b.body, ...flags([['--title', b.title]])];
  return c.json(await mycWrite(args));
});

api.post('/followups/:id/status', async (c) => {
  const id = c.req.param('id');
  const { status, reason } = await c.req.json();
  const verb =
    status === 'in_progress'
      ? 'start'
      : status === 'done'
        ? 'done'
        : status === 'wontfix'
          ? 'wontfix'
          : 'reopen';
  const args = ['followup', verb, id, ...flags([['--reason', reason]])];
  await mycExec(args); // status verbs emit a message, not JSON
  return c.json(await mycRead(['followup', 'show', id]));
});

api.patch('/followups/:id', async (c) => {
  const id = c.req.param('id');
  const { body, title } = await c.req.json();
  const args = ['followup', 'edit', id];
  if (body !== undefined) args.push('--body', body);
  if (title === null) args.push('--title', '-');
  else if (title !== undefined) args.push('--title', title);
  await mycExec(args); // edit emits a message, not JSON
  return c.json(await mycRead(['followup', 'show', id]));
});

api.post('/followups/:id/append', async (c) => {
  const id = c.req.param('id');
  const { text } = await c.req.json();
  await mycExec(['followup', 'append', id, text]); // append emits a message
  return c.json(await mycRead(['followup', 'show', id]));
});

api.delete('/followups/:id', async (c) => {
  await mycExec(['followup', 'rm', c.req.param('id'), '--force']);
  return c.json({ ok: true });
});

app.route('/api', api);

// --- static frontend (built React app) --------------------------------------
app.use('/*', serveStatic({ root: './public' }));
app.get('/*', serveStatic({ path: './public/index.html' })); // SPA fallback

const port = Number(process.env.PORT || 8787);
console.log(`mycelium-web on :${port}  (project: ${projectDir()})`);

export default { port, fetch: app.fetch };
