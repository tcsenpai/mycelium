import { invoke } from '@tauri-apps/api/core';
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

export async function openFolderDialog(): Promise<string | null> {
  return await invoke('open_folder_dialog');
}

export async function openFolder(path: string): Promise<void> {
  return await invoke('open_folder', { path });
}

export async function getCurrentDbPath(): Promise<string | null> {
  return await invoke('get_current_db_path');
}

export async function getRecentFolders(): Promise<string[]> {
  return await invoke('get_recent_folders');
}

export async function getDashboardStats(): Promise<DashboardStats> {
  return invoke('get_dashboard_stats');
}

export async function getTasks(filters: TaskFilters = {}): Promise<Task[]> {
  return invoke('get_tasks', { filters });
}

export async function getTask(id: number): Promise<Task | null> {
  return invoke('get_task', { id });
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
  return invoke('create_task', { task });
}

export async function updateTask(id: number, updates: TaskUpdateInput): Promise<Task> {
  return invoke('update_task', { id, updates });
}

export async function deleteTask(id: number): Promise<void> {
  return invoke('delete_task', { id });
}

export async function closeTask(id: number): Promise<Task> {
  return await invoke('close_task', { id });
}

export async function startTask(id: number): Promise<Task> {
  return await invoke('start_task', { id });
}

export async function reopenTask(id: number): Promise<Task> {
  return invoke('reopen_task', { id });
}

export async function getEpics(): Promise<Epic[]> {
  return invoke('get_epics');
}

export async function getEpic(id: number): Promise<Epic | null> {
  return invoke('get_epic', { id });
}

export async function createEpic(epic: { title: string; description?: string }): Promise<Epic> {
  return invoke('create_epic', { epic });
}

export async function updateEpic(id: number, updates: Partial<Epic>): Promise<Epic> {
  return invoke('update_epic', { id, updates });
}

export async function deleteEpic(id: number): Promise<void> {
  return invoke('delete_epic', { id });
}

export async function getAssignees(): Promise<Assignee[]> {
  return invoke('get_assignees');
}

export async function createAssignee(assignee: { name: string; email?: string; github_username?: string }): Promise<Assignee> {
  return invoke('create_assignee', { assignee });
}

export async function addDependency(taskId: number, dependsOn: number): Promise<void> {
  return invoke('add_dependency', { taskId, dependsOn });
}

export async function removeDependency(taskId: number, dependsOn: number): Promise<void> {
  return invoke('remove_dependency', { taskId, dependsOn });
}

export async function searchTasks(query: string): Promise<Task[]> {
  return invoke('search_tasks', { query });
}

export async function getAllTags(): Promise<string[]> {
  return invoke('get_all_tags');
}

// Follow-ups

export async function listFollowups(includeClosed = false): Promise<Followup[]> {
  return invoke('list_followups', { includeClosed });
}

export async function getFollowup(id: number): Promise<Followup | null> {
  return invoke('get_followup', { id });
}

export async function createFollowup(followup: { body: string; title?: string }): Promise<Followup> {
  return invoke('create_followup', { followup });
}

export async function setFollowupStatus(
  id: number,
  status: FollowupStatus,
  reason?: string,
): Promise<Followup> {
  return invoke('set_followup_status', { id, status, reason });
}

export async function updateFollowup(
  id: number,
  body?: string,
  title?: string | null,
): Promise<Followup> {
  // title === undefined means leave alone; null means clear; string means set
  const payload: { id: number; body?: string; title?: string | null | undefined } = { id };
  if (body !== undefined) payload.body = body;
  if (title !== undefined) payload.title = title;
  return invoke('update_followup', payload);
}

export async function appendFollowup(id: number, text: string): Promise<Followup> {
  return invoke('append_followup', { id, text });
}

export async function deleteFollowup(id: number): Promise<void> {
  return invoke('delete_followup', { id });
}

export async function countFollowups(): Promise<FollowupCounts> {
  return invoke('count_followups');
}
