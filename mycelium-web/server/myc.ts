// Single choke point for invoking the `myc` CLI.
// Every API route funnels through runMyc(): spawn, capture, parse JSON, map errors.
// Writes are serialized through a process-wide mutex so two rapid mutations
// cannot race the same SQLite row.
// ponytail: one global write lock. Per-table locks only if throughput demands.

const MYC_BIN = process.env.MYC_BIN || 'myc';
const PROJECT_DIR = process.env.MYC_PROJECT_DIR || process.cwd();
const TIMEOUT_MS = Number(process.env.MYC_TIMEOUT_MS || 15_000);

export class MycError extends Error {
  constructor(
    message: string,
    readonly code: number,
    readonly stderr: string,
  ) {
    super(message);
    this.name = 'MycError';
  }
}

let writeChain: Promise<unknown> = Promise.resolve();

function runRaw(args: string[]): Promise<string> {
  const proc = Bun.spawn([MYC_BIN, ...args, '--format', 'json'], {
    cwd: PROJECT_DIR,
    stdout: 'pipe',
    stderr: 'pipe',
  });

  const timer = setTimeout(() => proc.kill(), TIMEOUT_MS);

  return (async () => {
    try {
      const [stdout, stderr, code] = await Promise.all([
        new Response(proc.stdout).text(),
        new Response(proc.stderr).text(),
        proc.exited,
      ]);
      if (code !== 0) {
        const msg = stderr.trim() || stdout.trim() || `myc exited with ${code}`;
        throw new MycError(msg, code, stderr);
      }
      return stdout;
    } finally {
      clearTimeout(timer);
    }
  })();
}

// The myc CLI serializes the `in_progress` enum as "inprogress" (dropped
// underscore) in JSON, inconsistent with what it accepts on input and with
// the frontend's Status type. Normalize every `status` field on read.
function normalizeStatus(node: unknown): void {
  if (Array.isArray(node)) {
    for (const item of node) normalizeStatus(item);
  } else if (node && typeof node === 'object') {
    const obj = node as Record<string, unknown>;
    if (obj.status === 'inprogress') obj.status = 'in_progress';
    for (const v of Object.values(obj)) normalizeStatus(v);
  }
}

function parse<T>(raw: string): T {
  const trimmed = raw.trim();
  if (!trimmed) return undefined as T;
  let value: unknown;
  try {
    value = JSON.parse(trimmed);
  } catch {
    throw new MycError(`myc returned non-JSON output: ${trimmed.slice(0, 200)}`, 0, '');
  }
  normalizeStatus(value);
  return value as T;
}

// Read: no lock needed (WAL handles concurrent reads).
export async function mycRead<T>(args: string[]): Promise<T> {
  return parse<T>(await runRaw(args));
}

// Serialize a write through the global chain, returning raw stdout.
function serialized(args: string[]): Promise<string> {
  const run = writeChain.then(() => runRaw(args));
  // keep the chain alive regardless of this call's success
  writeChain = run.then(
    () => undefined,
    () => undefined,
  );
  return run;
}

// Write that returns a JSON object (create/update commands).
export async function mycWrite<T>(args: string[]): Promise<T> {
  return parse<T>(await serialized(args));
}

// Write that returns a human message, not JSON (close/reopen/delete/link/
// followup status transitions). Runs serialized, discards stdout.
export async function mycExec(args: string[]): Promise<void> {
  await serialized(args);
}

export function projectDir(): string {
  return PROJECT_DIR;
}
