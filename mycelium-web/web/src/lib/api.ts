// HTTP client for the mycelium-web server. Mirrors the original Tauri
// invoke()-based api.ts one-for-one so App.tsx is reused verbatim.

import type {
  Task,
  Epic,
  Assignee,
  DashboardStats,
  TaskFilters,
  Followup,
  FollowupCounts,
  FollowupStatus,
} from './types';

const BASE = '/api';

async function http<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(BASE + path, {
    headers: { 'Content-Type': 'application/json' },
    ...init,
  });
  if (!res.ok) {
    let msg = `Request failed (${res.status})`;
    try {
      const body = await res.json();
      if (body?.error) msg = body.error;
    } catch {
      /* keep default */
    }
    throw new Error(msg);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

const get = <T>(path: string) => http<T>(path);
const post = <T>(path: string, body?: unknown) =>
  http<T>(path, { method: 'POST', body: body === undefined ? undefined : JSON.stringify(body) });
const patch = <T>(path: string, body: unknown) =>
  http<T>(path, { method: 'PATCH', body: JSON.stringify(body) });
const del = <T>(path: string) => http<T>(path, { method: 'DELETE' });

export type TaskUpdateInput = {
  title?: string;
  description?: string;
  status?: Task['status'];
  priority?: Task['priority'];
  epic_id?: number | null;
  assignee_id?: number | null;
  due_date?: string | null;
  tags?: string | null;
};

// --- project meta (web: single fixed project; folder-picker is a no-op) ------
export async function openFolderDialog(): Promise<string | null> {
  return null;
}
export async function openFolder(_path: string): Promise<void> {
  /* no-op on web */
}
export async function getCurrentDbPath(): Promise<string | null> {
  return get<string | null>('/current-db-path');
}
export async function getRecentFolders(): Promise<string[]> {
  return get<string[]>('/recent-folders');
}

// --- dashboard ---------------------------------------------------------------
export async function getDashboardStats(): Promise<DashboardStats> {
  return get<DashboardStats>('/summary');
}

// --- tasks -------------------------------------------------------------------
function taskQuery(filters: TaskFilters): string {
  const p = new URLSearchParams();
  if (filters.epic_id != null) p.set('epic_id', String(filters.epic_id));
  if (filters.status) p.set('status', filters.status);
  if (filters.priority) p.set('priority', filters.priority);
  if (filters.assignee_id != null) p.set('assignee_id', String(filters.assignee_id));
  if (filters.tag) p.set('tag', filters.tag);
  if (filters.blocked) p.set('blocked', 'true');
  if (filters.overdue) p.set('overdue', 'true');
  if (filters.search) p.set('search', filters.search);
  const qs = p.toString();
  return qs ? `?${qs}` : '';
}

export async function getTasks(filters: TaskFilters = {}): Promise<Task[]> {
  return get<Task[]>(`/tasks${taskQuery(filters)}`);
}
export async function getTask(id: number): Promise<Task | null> {
  return get<Task | null>(`/tasks/${id}`);
}
export async function createTask(task: {
  title: string;
  description?: string;
  epic_id?: number;
  priority: string;
  assignee_id?: number;
  due_date?: string;
  tags?: string;
}): Promise<Task> {
  return post<Task>('/tasks', task);
}
export async function updateTask(id: number, updates: TaskUpdateInput): Promise<Task> {
  return patch<Task>(`/tasks/${id}`, updates);
}
export async function deleteTask(id: number): Promise<void> {
  await del(`/tasks/${id}`);
}
export async function closeTask(id: number): Promise<Task> {
  return post<Task>(`/tasks/${id}/close`);
}
export async function startTask(id: number): Promise<Task> {
  return post<Task>(`/tasks/${id}/start`);
}
export async function reopenTask(id: number): Promise<Task> {
  return post<Task>(`/tasks/${id}/reopen`);
}
export async function searchTasks(query: string): Promise<Task[]> {
  return get<Task[]>(`/search?q=${encodeURIComponent(query)}`);
}
export async function getAllTags(): Promise<string[]> {
  return get<string[]>('/tags');
}

// --- epics -------------------------------------------------------------------
export async function getEpics(): Promise<Epic[]> {
  return get<Epic[]>('/epics');
}
export async function getEpic(id: number): Promise<Epic | null> {
  return get<Epic | null>(`/epics/${id}`);
}
export async function createEpic(epic: { title: string; description?: string }): Promise<Epic> {
  return post<Epic>('/epics', epic);
}
export async function updateEpic(id: number, updates: Partial<Epic>): Promise<Epic> {
  return patch<Epic>(`/epics/${id}`, updates);
}
export async function deleteEpic(id: number): Promise<void> {
  await del(`/epics/${id}`);
}

// --- assignees ---------------------------------------------------------------
export async function getAssignees(): Promise<Assignee[]> {
  return get<Assignee[]>('/assignees');
}
export async function createAssignee(assignee: {
  name: string;
  email?: string;
  github_username?: string;
}): Promise<Assignee> {
  return post<Assignee>('/assignees', assignee);
}

// --- deps --------------------------------------------------------------------
export async function addDependency(taskId: number, dependsOn: number): Promise<void> {
  await post(`/tasks/${taskId}/deps`, { depends_on: dependsOn });
}
export async function removeDependency(taskId: number, dependsOn: number): Promise<void> {
  await del(`/tasks/${taskId}/deps/${dependsOn}`);
}

// --- followups ---------------------------------------------------------------
export async function listFollowups(includeClosed = false): Promise<Followup[]> {
  return get<Followup[]>(`/followups?includeClosed=${includeClosed}`);
}
export async function getFollowup(id: number): Promise<Followup | null> {
  return get<Followup | null>(`/followups/${id}`);
}
export async function createFollowup(followup: { body: string; title?: string }): Promise<Followup> {
  return post<Followup>('/followups', followup);
}
export async function setFollowupStatus(
  id: number,
  status: FollowupStatus,
  reason?: string,
): Promise<Followup> {
  return post<Followup>(`/followups/${id}/status`, { status, reason });
}
export async function updateFollowup(
  id: number,
  body?: string,
  title?: string | null,
): Promise<Followup> {
  const payload: { body?: string; title?: string | null } = {};
  if (body !== undefined) payload.body = body;
  if (title !== undefined) payload.title = title;
  return patch<Followup>(`/followups/${id}`, payload);
}
export async function appendFollowup(id: number, text: string): Promise<Followup> {
  return post<Followup>(`/followups/${id}/append`, { text });
}
export async function deleteFollowup(id: number): Promise<void> {
  await del(`/followups/${id}`);
}
export async function countFollowups(): Promise<FollowupCounts> {
  return get<FollowupCounts>('/followups/count');
}
