export type Status = 'open' | 'in_progress' | 'closed';

export type Priority = 'low' | 'medium' | 'high' | 'critical';

export interface Task {
  id: number;
  title: string;
  description?: string;
  status: Status;
  priority: Priority;
  epic_id?: number;
  epic_title?: string;
  assignee_id?: number;
  assignee_name?: string;
  due_date?: string;
  tags?: string;
  created_at: string;
  updated_at: string;
  blocked_by: number[];
  blocks: number[];
}

export interface Epic {
  id: number;
  title: string;
  description?: string;
  status: Status;
  total_tasks: number;
  open_tasks: number;
  created_at: string;
  updated_at: string;
}

export interface Assignee {
  id: number;
  name: string;
  email?: string;
  github_username?: string;
  total_tasks: number;
  open_tasks: number;
}

export interface DashboardStats {
  total_epics: number;
  open_epics: number;
  closed_epics: number;
  total_tasks: number;
  open_tasks: number;
  closed_tasks: number;
  overdue_tasks: number;
  blocked_tasks: number;
  high_priority_open: number;
  completion_rate: number;
}

export interface TaskFilters {
  epic_id?: number;
  status?: Status;
  priority?: Priority;
  assignee_id?: number;
  tag?: string;
  blocked?: boolean;
  overdue?: boolean;
  search?: string;
}

export const priorityColors: Record<Priority, string> = {
  low: 'bg-blue-500',
  medium: 'bg-green-500',
  high: 'bg-orange-500',
  critical: 'bg-red-500',
};

export const priorityLabels: Record<Priority, string> = {
  low: 'Low',
  medium: 'Medium',
  high: 'High',
  critical: 'Critical',
};

export type FollowupStatus = 'open' | 'in_progress' | 'done' | 'wontfix';

export interface Followup {
  id: number;
  body: string;
  title?: string | null;
  status: FollowupStatus;
  closure_reason?: string | null;
  created_at: string;
  closed_at?: string | null;
}

export interface FollowupCounts {
  open: number;
  in_progress: number;
  done: number;
  wontfix: number;
}

export const followupStatusLabels: Record<FollowupStatus, string> = {
  open: 'Open',
  in_progress: 'In Progress',
  done: 'Done',
  wontfix: 'Won’t Fix',
};

// Map to hand-written CSS modifier classes (see .status-pill.is-* in index.css).
// NOT Tailwind — this project has no Tailwind; class names must match real CSS.
export const followupStatusColors: Record<FollowupStatus, string> = {
  open: 'is-open',
  in_progress: 'is-in-progress',
  done: 'is-done',
  wontfix: 'is-wontfix',
};
