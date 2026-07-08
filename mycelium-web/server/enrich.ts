// Task enrichment: `myc task list` returns bare rows without join fields
// (epic_title, assignee_name) or dependency arrays (blocked_by, blocks).
// The frontend Task shape needs all four. `myc task show <id>` returns the
// fully-joined shape, so we fan out show-calls with a concurrency cap.

import { mycRead } from './myc';

const SHOW_CONCURRENCY = 8;

export interface BareTask {
  id: number;
  [k: string]: unknown;
}

export interface EnrichedTask extends BareTask {
  epic_title: string | null;
  assignee_name: string | null;
  blocked_by: number[];
  blocks: number[];
}

interface ShowResult {
  task: Record<string, unknown>;
  epic_title: string | null;
  assignee_name: string | null;
  blocked_by: number[];
  blocks: number[];
  external_refs: unknown[];
}

async function showTask(id: number): Promise<EnrichedTask> {
  const s = await mycRead<ShowResult>(['task', 'show', String(id)]);
  return {
    ...s.task,
    id,
    epic_title: s.epic_title ?? null,
    assignee_name: s.assignee_name ?? null,
    blocked_by: s.blocked_by ?? [],
    blocks: s.blocks ?? [],
  } as EnrichedTask;
}

// Fan out task show calls with a simple sliding-window concurrency cap.
export async function enrichTasks(ids: number[]): Promise<EnrichedTask[]> {
  const out: EnrichedTask[] = new Array(ids.length);
  let cursor = 0;

  async function worker() {
    while (cursor < ids.length) {
      const i = cursor++;
      out[i] = await showTask(ids[i]);
    }
  }

  const workers = Array.from(
    { length: Math.min(SHOW_CONCURRENCY, ids.length) },
    () => worker(),
  );
  await Promise.all(workers);
  return out;
}
