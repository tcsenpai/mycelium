import {
  startTransition,
  useDeferredValue,
  useEffect,
  useRef,
  useState,
} from 'react';
import { listen } from '@tauri-apps/api/event';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  AlertTriangle,
  ArrowRight,
  BarChart3,
  Briefcase,
  CalendarDays,
  CheckCheck,
  ChevronsUpDown,
  CircleDot,
  FolderOpen,
  Layers3,
  Loader,
  Plus,
  Search,
  Sparkles,
  X,
} from 'lucide-react';
import {
  closeTask,
  createTask,
  getAllTags,
  getAssignees,
  getCurrentDbPath,
  getDashboardStats,
  getEpics,
  getRecentFolders,
  getTasks,
  openFolder,
  openFolderDialog,
  reopenTask,
  updateTask,
} from './lib/api';
import type { TaskUpdateInput } from './lib/api';
import type { Priority, Task } from './lib/types';

const QUERY_KEYS = {
  path: ['current-path'],
  recent: ['recent-folders'],
  stats: ['stats'],
  tasks: ['tasks'],
  epics: ['epics'],
  assignees: ['assignees'],
  tags: ['tags'],
};

type StatusFilter = 'all' | 'open' | 'closed' | 'blocked' | 'overdue';
type PriorityFilter = 'all' | Priority;
type WorkspaceMode = 'overview' | 'analytics';
type SmartFilter = 'all' | 'focus' | 'up-next' | 'unassigned' | 'unscheduled' | 'recent';
type DueFilter = 'all' | 'overdue' | 'today' | 'soon' | 'none';
type SortMode = 'priority' | 'due' | 'updated' | 'title';
type AnalyticsWindow = 7 | 30 | 90;

type ComposerState = {
  title: string;
  description: string;
  epicId: string;
  priority: Priority;
  assigneeId: string;
  dueDate: string;
  tags: string;
};

const initialComposerState: ComposerState = {
  title: '',
  description: '',
  epicId: '',
  priority: 'medium',
  assigneeId: '',
  dueDate: '',
  tags: '',
};

type DependencyGraphNode = {
  id: number;
  title: string;
  priority: Priority;
  status: Task['status'];
  epicTitle?: string;
  blockedBy: number[];
  blocks: number[];
  layer: number;
  x: number;
  y: number;
};

type DependencyGraphEdge = {
  from: number;
  to: number;
};

type MiniPanelItem = {
  id?: number;
  label: string;
  meta: string;
  onClick?: () => void;
};

type DetailDraft = {
  title: string;
  description: string;
  tags: string;
};

function priorityRank(priority: Priority) {
  switch (priority) {
    case 'critical':
      return 0;
    case 'high':
      return 1;
    case 'medium':
      return 2;
    case 'low':
      return 3;
  }
}

function priorityLabel(priority: Priority) {
  return priority.charAt(0).toUpperCase() + priority.slice(1);
}

function taskTone(priority: Priority) {
  switch (priority) {
    case 'critical':
      return 'task-card task-card-critical';
    case 'high':
      return 'task-card task-card-high';
    case 'medium':
      return 'task-card task-card-medium';
    case 'low':
      return 'task-card task-card-low';
  }
}

function formatDate(date?: string) {
  if (!date) {
    return 'No date';
  }

  const parsed = new Date(date);
  if (Number.isNaN(parsed.getTime())) {
    return date;
  }

  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    year: parsed.getFullYear() !== new Date().getFullYear() ? 'numeric' : undefined,
  }).format(parsed);
}

function dueState(task: Task) {
  if (!task.due_date || task.status === 'closed') {
    return null;
  }

  const today = new Date();
  today.setHours(0, 0, 0, 0);

  const due = new Date(task.due_date);
  due.setHours(0, 0, 0, 0);

  const diffDays = Math.round((due.getTime() - today.getTime()) / 86_400_000);
  if (diffDays < 0) {
    return { label: `${Math.abs(diffDays)}d late`, tone: 'is-danger' };
  }
  if (diffDays === 0) {
    return { label: 'Due today', tone: 'is-warning' };
  }
  if (diffDays <= 3) {
    return { label: `${diffDays}d left`, tone: 'is-warning' };
  }

  return { label: formatDate(task.due_date), tone: 'is-muted' };
}

function dueDelta(task: Task) {
  if (!task.due_date) {
    return null;
  }

  const today = new Date();
  today.setHours(0, 0, 0, 0);

  const due = new Date(task.due_date);
  due.setHours(0, 0, 0, 0);

  return Math.round((due.getTime() - today.getTime()) / 86_400_000);
}

function isRecentlyTouched(task: Task) {
  const updatedAt = new Date(task.updated_at).getTime();
  return Date.now() - updatedAt <= 1000 * 60 * 60 * 24 * 7;
}

function firstTag(tags?: string) {
  return tags?.split(',').map((tag) => tag.trim()).filter(Boolean)[0] ?? null;
}

function stripMarkdown(markdown?: string) {
  if (!markdown) {
    return '';
  }

  return markdown
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/`([^`]+)`/g, '$1')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/\*([^*]+)\*/g, '$1')
    .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
    .replace(/^#{1,6}\s+/gm, '')
    .replace(/^\s*[-*+]\s+/gm, '')
    .replace(/\n+/g, ' ')
    .trim();
}

function renderInlineMarkdown(text: string, keyPrefix: string) {
  const parts = text.split(/(`[^`]+`|\*\*[^*]+\*\*|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g).filter(Boolean);
  return parts.map((part, index) => {
    const key = `${keyPrefix}-${index}`;

    if (part.startsWith('`') && part.endsWith('`')) {
      return <code key={key}>{part.slice(1, -1)}</code>;
    }

    if (part.startsWith('**') && part.endsWith('**')) {
      return <strong key={key}>{part.slice(2, -2)}</strong>;
    }

    if (part.startsWith('*') && part.endsWith('*')) {
      return <em key={key}>{part.slice(1, -1)}</em>;
    }

    const linkMatch = part.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
    if (linkMatch) {
      return (
        <a key={key} href={linkMatch[2]} target="_blank" rel="noreferrer">
          {linkMatch[1]}
        </a>
      );
    }

    return <span key={key}>{part}</span>;
  });
}

function MarkdownBlock({ content }: { content?: string }) {
  if (!content?.trim()) {
    return <p>No description yet.</p>;
  }

  const lines = content.replace(/\r\n/g, '\n').split('\n');
  const nodes: React.ReactNode[] = [];
  let paragraph: string[] = [];
  let listItems: string[] = [];
  let codeLines: string[] = [];
  let inCode = false;

  const flushParagraph = () => {
    if (!paragraph.length) return;
    const text = paragraph.join(' ').trim();
    nodes.push(<p key={`p-${nodes.length}`}>{renderInlineMarkdown(text, `p-${nodes.length}`)}</p>);
    paragraph = [];
  };

  const flushList = () => {
    if (!listItems.length) return;
    nodes.push(
      <ul key={`ul-${nodes.length}`}>
        {listItems.map((item, index) => (
          <li key={`li-${index}`}>{renderInlineMarkdown(item, `li-${index}`)}</li>
        ))}
      </ul>
    );
    listItems = [];
  };

  const flushCode = () => {
    if (!codeLines.length) return;
    nodes.push(
      <pre key={`pre-${nodes.length}`}>
        <code>{codeLines.join('\n')}</code>
      </pre>
    );
    codeLines = [];
  };

  lines.forEach((line) => {
    if (line.trim().startsWith('```')) {
      flushParagraph();
      flushList();
      if (inCode) flushCode();
      inCode = !inCode;
      return;
    }

    if (inCode) {
      codeLines.push(line);
      return;
    }

    if (!line.trim()) {
      flushParagraph();
      flushList();
      return;
    }

    const headingMatch = line.match(/^(#{1,6})\s+(.*)$/);
    if (headingMatch) {
      flushParagraph();
      flushList();
      const level = Math.min(headingMatch[1].length + 1, 6);
      const text = headingMatch[2];
      if (level === 2) nodes.push(<h2 key={`h-${nodes.length}`}>{renderInlineMarkdown(text, `h-${nodes.length}`)}</h2>);
      else if (level === 3) nodes.push(<h3 key={`h-${nodes.length}`}>{renderInlineMarkdown(text, `h-${nodes.length}`)}</h3>);
      else nodes.push(<h4 key={`h-${nodes.length}`}>{renderInlineMarkdown(text, `h-${nodes.length}`)}</h4>);
      return;
    }

    const listMatch = line.match(/^\s*[-*+]\s+(.*)$/);
    if (listMatch) {
      flushParagraph();
      listItems.push(listMatch[1]);
      return;
    }

    paragraph.push(line.trim());
  });

  flushParagraph();
  flushList();
  flushCode();

  return <div className="markdown-block">{nodes}</div>;
}

function describeError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === 'string') {
    return error;
  }

  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string') {
      return message;
    }
  }

  try {
    return JSON.stringify(error);
  } catch {
    return 'Unknown error';
  }
}

function App() {
  const queryClient = useQueryClient();
  const composerTitleRef = useRef<HTMLInputElement>(null);
  const shellRef = useRef<HTMLDivElement>(null);
  const workspaceGridRef = useRef<HTMLElement>(null);

  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [priorityFilter, setPriorityFilter] = useState<PriorityFilter>('all');
  const [smartFilter, setSmartFilter] = useState<SmartFilter>('all');
  const [dueFilter, setDueFilter] = useState<DueFilter>('all');
  const [sortMode, setSortMode] = useState<SortMode>('priority');
  const [workspaceMode, setWorkspaceMode] = useState<WorkspaceMode>('overview');
  const [analyticsWindow, setAnalyticsWindow] = useState<AnalyticsWindow>(30);
  const [selectedEpicId, setSelectedEpicId] = useState<number | null>(null);
  const [queueEpicFilter, setQueueEpicFilter] = useState('all');
  const [queueAssigneeFilter, setQueueAssigneeFilter] = useState('all');
  const [queueTagFilter, setQueueTagFilter] = useState('all');
  const [selectedTaskId, setSelectedTaskId] = useState<number | null>(null);
  const [hoveredDependencyNodeId, setHoveredDependencyNodeId] = useState<number | null>(null);
  const [search, setSearch] = useState('');
  const deferredSearch = useDeferredValue(search.trim().toLowerCase());
  const [composerOpen, setComposerOpen] = useState(false);
  const [composer, setComposer] = useState<ComposerState>(initialComposerState);
  const [detailDraft, setDetailDraft] = useState<DetailDraft>({ title: '', description: '', tags: '' });
  const [railWidth, setRailWidth] = useState(320);
  const [detailWidth, setDetailWidth] = useState(360);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const pathQuery = useQuery({
    queryKey: QUERY_KEYS.path,
    queryFn: getCurrentDbPath,
    placeholderData: (previous) => previous,
  });

  const recentFoldersQuery = useQuery({
    queryKey: QUERY_KEYS.recent,
    queryFn: getRecentFolders,
    placeholderData: (previous) => previous,
  });

  const statsQuery = useQuery({
    queryKey: QUERY_KEYS.stats,
    queryFn: getDashboardStats,
    refetchInterval: 10_000,
    placeholderData: (previous) => previous,
  });

  const tasksQuery = useQuery({
    queryKey: QUERY_KEYS.tasks,
    queryFn: () => getTasks({}),
    refetchInterval: 10_000,
    placeholderData: (previous) => previous,
  });

  const epicsQuery = useQuery({
    queryKey: QUERY_KEYS.epics,
    queryFn: getEpics,
    refetchInterval: 15_000,
    placeholderData: (previous) => previous,
  });

  const assigneesQuery = useQuery({
    queryKey: QUERY_KEYS.assignees,
    queryFn: getAssignees,
    refetchInterval: 30_000,
    placeholderData: (previous) => previous,
  });

  const tagsQuery = useQuery({
    queryKey: QUERY_KEYS.tags,
    queryFn: getAllTags,
    refetchInterval: 30_000,
    placeholderData: (previous) => previous,
  });

  const openProjectMutation = useMutation({
    mutationFn: async (path: string) => {
      await openFolder(path);
    },
    onSuccess: async () => {
      setErrorMessage(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.path }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.recent }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.stats }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tasks }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.epics }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.assignees }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tags }),
      ]);
    },
    onError: (error) => {
      setErrorMessage(describeError(error));
    },
  });

  const createTaskMutation = useMutation({
    mutationFn: async (draft: ComposerState) =>
      createTask({
        title: draft.title,
        description: draft.description || undefined,
        epic_id: draft.epicId ? Number(draft.epicId) : undefined,
        priority: draft.priority,
        assignee_id: draft.assigneeId ? Number(draft.assigneeId) : undefined,
        due_date: draft.dueDate || undefined,
        tags: draft.tags || undefined,
      }),
    onSuccess: async (task) => {
      setComposer(initialComposerState);
      setComposerOpen(false);
      startTransition(() => setSelectedTaskId(task.id));
      setErrorMessage(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.stats }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tasks }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tags }),
      ]);
    },
    onError: (error) => {
      setErrorMessage(describeError(error));
    },
  });

  const toggleTaskMutation = useMutation({
    mutationFn: async (task: Task) => {
      if (task.status === 'closed') {
        return reopenTask(task.id);
      }
      return closeTask(task.id);
    },
    onSuccess: async (task) => {
      startTransition(() => setSelectedTaskId(task.id));
      setErrorMessage(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.stats }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tasks }),
      ]);
    },
    onError: (error) => {
      setErrorMessage(describeError(error));
    },
  });

  const updateTaskMutation = useMutation({
    mutationFn: async ({ id, updates }: { id: number; updates: TaskUpdateInput }) =>
      updateTask(id, updates),
    onSuccess: async (task) => {
      startTransition(() => setSelectedTaskId(task.id));
      setErrorMessage(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.stats }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tasks }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.epics }),
        queryClient.invalidateQueries({ queryKey: QUERY_KEYS.assignees }),
      ]);
    },
    onError: (error) => {
      setErrorMessage(describeError(error));
    },
  });

  useEffect(() => {
    const registerEvents = async () => {
      const unlistenDb = await listen('database-changed', async () => {
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.path }),
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.recent }),
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.stats }),
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tasks }),
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.epics }),
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.assignees }),
          queryClient.invalidateQueries({ queryKey: QUERY_KEYS.tags }),
        ]);
      });

      const unlistenQuickAdd = await listen('quick-add', () => {
        setComposerOpen(true);
        requestAnimationFrame(() => composerTitleRef.current?.focus());
      });

      return () => {
        unlistenDb();
        unlistenQuickAdd();
      };
    };

    let cleanup: (() => void) | undefined;
    registerEvents().then((dispose) => {
      cleanup = dispose;
    });

    return () => {
      cleanup?.();
    };
  }, [queryClient]);

  useEffect(() => {
    const queryError =
      tasksQuery.error ??
      statsQuery.error ??
      epicsQuery.error ??
      assigneesQuery.error ??
      tagsQuery.error ??
      pathQuery.error ??
      recentFoldersQuery.error;

    if (!queryError) {
      return;
    }

    console.error('MycUI query error', queryError);
    setErrorMessage(describeError(queryError));
  }, [
    assigneesQuery.error,
    epicsQuery.error,
    pathQuery.error,
    recentFoldersQuery.error,
    statsQuery.error,
    tagsQuery.error,
    tasksQuery.error,
  ]);

  useEffect(() => {
    if (composerOpen) {
      requestAnimationFrame(() => composerTitleRef.current?.focus());
    }
  }, [composerOpen]);

  useEffect(() => {
    if (!composerOpen) {
      return;
    }

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setComposerOpen(false);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [composerOpen]);

  const tasks = tasksQuery.data ?? [];
  const epics = epicsQuery.data ?? [];
  const assignees = assigneesQuery.data ?? [];
  const tags = tagsQuery.data ?? [];
  const stats = statsQuery.data;
  const projectPath = pathQuery.data ?? null;
  const recentFolders = recentFoldersQuery.data ?? [];

  let visibleTasks = [...tasks];

  if (selectedEpicId !== null) {
    visibleTasks = visibleTasks.filter((task) => task.epic_id === selectedEpicId);
  }

  if (queueEpicFilter !== 'all') {
    visibleTasks = visibleTasks.filter((task) => task.epic_id === Number(queueEpicFilter));
  }

  if (priorityFilter !== 'all') {
    visibleTasks = visibleTasks.filter((task) => task.priority === priorityFilter);
  }

  if (queueAssigneeFilter !== 'all') {
    if (queueAssigneeFilter === 'none') {
      visibleTasks = visibleTasks.filter((task) => !task.assignee_id);
    } else {
      visibleTasks = visibleTasks.filter((task) => task.assignee_id === Number(queueAssigneeFilter));
    }
  }

  if (queueTagFilter !== 'all') {
    visibleTasks = visibleTasks.filter((task) =>
      task.tags
        ?.split(',')
        .map((tag) => tag.trim().toLowerCase())
        .includes(queueTagFilter.toLowerCase()) ?? false
    );
  }

  if (statusFilter === 'open' || statusFilter === 'closed') {
    visibleTasks = visibleTasks.filter((task) => task.status === statusFilter);
  }

  if (statusFilter === 'blocked') {
    visibleTasks = visibleTasks.filter((task) => task.status === 'open' && task.blocked_by.length > 0);
  }

  if (statusFilter === 'overdue') {
    visibleTasks = visibleTasks.filter((task) => {
      const due = task.due_date ? new Date(task.due_date) : null;
      return task.status === 'open' && due !== null && due.getTime() < Date.now();
    });
  }

  if (dueFilter !== 'all') {
    visibleTasks = visibleTasks.filter((task) => {
      const delta = dueDelta(task);

      if (dueFilter === 'none') {
        return delta === null;
      }

      if (delta === null || task.status === 'closed') {
        return false;
      }

      if (dueFilter === 'overdue') {
        return delta < 0;
      }

      if (dueFilter === 'today') {
        return delta === 0;
      }

      return delta > 0 && delta <= 3;
    });
  }

  if (deferredSearch) {
    visibleTasks = visibleTasks.filter((task) => {
      const haystack = [
        task.title,
        task.description,
        task.epic_title,
        task.assignee_name,
        task.tags,
      ]
        .filter(Boolean)
        .join(' ')
        .toLowerCase();

      return haystack.includes(deferredSearch);
    });
  }

  if (smartFilter !== 'all') {
    visibleTasks = visibleTasks.filter((task) => {
      const delta = dueDelta(task);

      switch (smartFilter) {
        case 'focus':
          return (
            task.status === 'open' &&
            (task.priority === 'critical' ||
              task.priority === 'high' ||
              task.blocked_by.length > 0 ||
              (delta !== null && delta <= 0))
          );
        case 'up-next':
          return (
            task.status === 'open' &&
            task.blocked_by.length === 0 &&
            ((delta !== null && delta >= 0 && delta <= 7) ||
              task.priority === 'critical' ||
              task.priority === 'high')
          );
        case 'unassigned':
          return !task.assignee_id;
        case 'unscheduled':
          return !task.due_date;
        case 'recent':
          return isRecentlyTouched(task);
      }
    });
  }

  visibleTasks.sort((a, b) => {
    if (a.status !== b.status) {
      return a.status === 'open' ? -1 : 1;
    }

    if (sortMode === 'title') {
      return a.title.localeCompare(b.title);
    }

    if (sortMode === 'updated') {
      return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
    }

    const aDue = a.due_date ? new Date(a.due_date).getTime() : Number.MAX_SAFE_INTEGER;
    const bDue = b.due_date ? new Date(b.due_date).getTime() : Number.MAX_SAFE_INTEGER;

    if (sortMode === 'due') {
      if (aDue !== bDue) {
        return aDue - bDue;
      }
      return priorityRank(a.priority) - priorityRank(b.priority);
    }

    if (aDue !== bDue) {
      return aDue - bDue;
    }

    const rankDiff = priorityRank(a.priority) - priorityRank(b.priority);
    if (rankDiff !== 0) {
      return rankDiff;
    }

    return b.id - a.id;
  });

  const selectedTask =
    visibleTasks.find((task) => task.id === selectedTaskId) ??
    tasks.find((task) => task.id === selectedTaskId) ??
    visibleTasks[0] ??
    null;

  useEffect(() => {
    setDetailDraft({
      title: selectedTask?.title ?? '',
      description: selectedTask?.description ?? '',
      tags: selectedTask?.tags ?? '',
    });
  }, [selectedTask?.id, selectedTask?.title, selectedTask?.description, selectedTask?.tags]);

  useEffect(() => {
    if (!selectedTaskId && visibleTasks[0]) {
      setSelectedTaskId(visibleTasks[0].id);
      return;
    }

    if (selectedTaskId && !tasks.some((task) => task.id === selectedTaskId)) {
      setSelectedTaskId(visibleTasks[0]?.id ?? null);
    }
  }, [selectedTaskId, tasks, visibleTasks]);

  const openTasks = tasks.filter((task) => task.status === 'open');
  const blockedTasks = openTasks.filter((task) => task.blocked_by.length > 0);
  const closedTasks = tasks.filter((task) => task.status === 'closed');
  const epicsWithStats = epics.map((epic) => {
    const epicTasks = tasks.filter((task) => task.epic_id === epic.id);
    const totalTasks = epicTasks.length;
    const openTaskCount = epicTasks.filter((task) => task.status === 'open').length;
    return {
      ...epic,
      total_tasks: totalTasks,
      open_tasks: openTaskCount,
    };
  });
  const highSignalTasks = openTasks
    .filter((task) => task.priority === 'high' || task.priority === 'critical')
    .slice(0, 3);
  const urgentTasks = openTasks
    .filter((task) => {
      const state = dueState(task);
      return state?.tone === 'is-danger' || state?.label === 'Due today';
    })
    .slice(0, 3);
  const activeFilterCount = [
    statusFilter !== 'all',
    priorityFilter !== 'all',
    smartFilter !== 'all',
    dueFilter !== 'all',
    queueEpicFilter !== 'all',
    queueAssigneeFilter !== 'all',
    queueTagFilter !== 'all',
    selectedEpicId !== null,
    Boolean(deferredSearch),
  ].filter(Boolean).length;

  const loading =
    tasksQuery.isLoading ||
    epicsQuery.isLoading ||
    assigneesQuery.isLoading ||
    statsQuery.isLoading ||
    pathQuery.isLoading ||
    recentFoldersQuery.isLoading;

  const summaryCards = [
    {
      label: 'Open work',
      value: stats?.open_tasks ?? 0,
      hint: `${stats?.total_tasks ?? 0} total tracked`,
      accent: 'sprout',
    },
    {
      label: 'Closed',
      value: stats?.closed_tasks ?? 0,
      hint: `${Math.round(stats?.completion_rate ?? 0)}% completion`,
      accent: 'ink',
    },
    {
      label: 'Overdue',
      value: stats?.overdue_tasks ?? 0,
      hint: 'Needs attention now',
      accent: 'ember',
    },
    {
      label: 'Blocked',
      value: stats?.blocked_tasks ?? 0,
      hint: 'Waiting on dependency',
      accent: 'gold',
    },
  ];

  const prioritySeries: { label: string; value: number; priority: Priority }[] = [
    { label: 'Critical', value: tasks.filter((task) => task.priority === 'critical').length, priority: 'critical' },
    { label: 'High', value: tasks.filter((task) => task.priority === 'high').length, priority: 'high' },
    { label: 'Medium', value: tasks.filter((task) => task.priority === 'medium').length, priority: 'medium' },
    { label: 'Low', value: tasks.filter((task) => task.priority === 'low').length, priority: 'low' },
  ];
  const maxPriorityValue = Math.max(1, ...prioritySeries.map((entry) => entry.value));
  const completionRate = Math.round(stats?.completion_rate ?? 0);
  const analyticsEpics = epicsWithStats
    .map((epic) => {
      const total = Math.max(epic.total_tasks, 1);
      const done = epic.total_tasks - epic.open_tasks;
      const pct = Math.round((done / total) * 100);
      return { ...epic, completion_pct: pct };
    })
    .sort((left, right) => {
      const leftRisk = left.open_tasks - left.completion_pct / 25;
      const rightRisk = right.open_tasks - right.completion_pct / 25;
      return rightRisk - leftRisk;
    })
    .slice(0, 6);
  const upcomingTasks = openTasks
    .filter((task) => Boolean(task.due_date))
    .sort((a, b) => new Date(a.due_date ?? '').getTime() - new Date(b.due_date ?? '').getTime())
    .slice(0, 6);
  const trendLength = analyticsWindow === 7 ? 7 : analyticsWindow === 30 ? 10 : 12;
  const trendStepDays = analyticsWindow === 7 ? 1 : analyticsWindow === 30 ? 3 : 7;
  const completionTrend = Array.from({ length: trendLength }, (_, index) => {
    const day = new Date();
    day.setHours(0, 0, 0, 0);
    day.setDate(day.getDate() - trendStepDays * (trendLength - 1 - index));
    const nextDay = new Date(day);
    nextDay.setDate(nextDay.getDate() + trendStepDays);

    const completed = closedTasks.filter((task) => {
      const updated = new Date(task.updated_at);
      return updated >= day && updated < nextDay;
    }).length;

    const created = tasks.filter((task) => {
      const createdAt = new Date(task.created_at);
      return createdAt >= day && createdAt < nextDay;
    }).length;

    return {
      label:
        analyticsWindow === 7
          ? new Intl.DateTimeFormat(undefined, { weekday: 'short' }).format(day)
          : new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(day),
      completed,
      created,
    };
  });
  const maxTrendValue = Math.max(1, ...completionTrend.flatMap((entry) => [entry.completed, entry.created]));
  const trendPointsCompleted = completionTrend
    .map((entry, index) => {
      const x = (index / Math.max(completionTrend.length - 1, 1)) * 100;
      const y = 100 - (entry.completed / maxTrendValue) * 100;
      return `${x},${y}`;
    })
    .join(' ');
  const trendPointsCreated = completionTrend
    .map((entry, index) => {
      const x = (index / Math.max(completionTrend.length - 1, 1)) * 100;
      const y = 100 - (entry.created / maxTrendValue) * 100;
      return `${x},${y}`;
    })
    .join(' ');
  const analyticsStart = new Date();
  analyticsStart.setHours(0, 0, 0, 0);
  analyticsStart.setDate(analyticsStart.getDate() - analyticsWindow);
  const recentOpenTasks = openTasks.filter((task) => new Date(task.updated_at) >= analyticsStart);
  const intakeCount = tasks.filter((task) => new Date(task.created_at) >= analyticsStart).length;
  const completedCount = closedTasks.filter((task) => new Date(task.updated_at) >= analyticsStart).length;
  const recentBlockedCount = recentOpenTasks.filter((task) => task.blocked_by.length > 0).length;
  const overdueOpenCount = openTasks.filter((task) => (dueDelta(task) ?? 1) < 0).length;
  const agingBuckets = [
    {
      label: 'Fresh',
      value: openTasks.filter((task) => {
        const ageDays = Math.floor((Date.now() - new Date(task.created_at).getTime()) / 86_400_000);
        return ageDays <= 7;
      }).length,
      tone: 'is-medium',
    },
    {
      label: 'Warm',
      value: openTasks.filter((task) => {
        const ageDays = Math.floor((Date.now() - new Date(task.created_at).getTime()) / 86_400_000);
        return ageDays > 7 && ageDays <= 21;
      }).length,
      tone: 'is-high',
    },
    {
      label: 'Stale',
      value: openTasks.filter((task) => {
        const ageDays = Math.floor((Date.now() - new Date(task.created_at).getTime()) / 86_400_000);
        return ageDays > 21;
      }).length,
      tone: 'is-critical',
    },
  ];
  const assigneeLoad = assignees
    .map((assignee) => {
      const load = openTasks.filter((task) => task.assignee_id === assignee.id);
      const critical = load.filter((task) => task.priority === 'critical' || task.priority === 'high').length;
      const blocked = load.filter((task) => task.blocked_by.length > 0).length;
      return {
        id: assignee.id,
        name: assignee.name,
        open: load.length,
        critical,
        blocked,
      };
    })
    .filter((assignee) => assignee.open > 0)
    .sort((left, right) => right.critical - left.critical || right.open - left.open)
    .slice(0, 6);
  const operationalSignals = [
    { label: `${analyticsWindow}d intake`, value: String(intakeCount), meta: 'tasks created' },
    { label: `${analyticsWindow}d completed`, value: String(completedCount), meta: 'tasks closed' },
    { label: 'Active blockers', value: String(recentBlockedCount), meta: 'open + blocked' },
    { label: 'Open overdue', value: String(overdueOpenCount), meta: 'needs intervention' },
  ];
  const taskMap = new Map(tasks.map((task) => [task.id, task]));
  const dependencyCandidates = tasks
    .filter((task) => task.blocked_by.length > 0 || task.blocks.length > 0)
    .sort((left, right) => {
      const signalDelta =
        right.blocked_by.length + right.blocks.length - (left.blocked_by.length + left.blocks.length);
      if (signalDelta !== 0) {
        return signalDelta;
      }
      if (left.status !== right.status) {
        return left.status === 'open' ? -1 : 1;
      }
      const priorityDelta = priorityRank(left.priority) - priorityRank(right.priority);
      if (priorityDelta !== 0) {
        return priorityDelta;
      }
      return left.id - right.id;
    })
    .slice(0, 12);
  const dependencyNodeIds = new Set(dependencyCandidates.map((task) => task.id));
  const dependencyLayerCache = new Map<number, number>();
  const resolveDependencyLayer = (taskId: number, trail = new Set<number>()): number => {
    if (dependencyLayerCache.has(taskId)) {
      return dependencyLayerCache.get(taskId) ?? 0;
    }
    if (trail.has(taskId)) {
      return 0;
    }
    const task = taskMap.get(taskId);
    if (!task) {
      return 0;
    }
    const blockers = task.blocked_by.filter((blockedId) => dependencyNodeIds.has(blockedId));
    if (blockers.length === 0) {
      dependencyLayerCache.set(taskId, 0);
      return 0;
    }
    trail.add(taskId);
    const layer = Math.max(...blockers.map((blockedId) => resolveDependencyLayer(blockedId, trail) + 1));
    trail.delete(taskId);
    dependencyLayerCache.set(taskId, layer);
    return layer;
  };
  const dependencyGraphNodesBase = dependencyCandidates.map((task) => ({
    id: task.id,
    title: task.title,
    priority: task.priority,
    status: task.status,
    epicTitle: task.epic_title,
    blockedBy: task.blocked_by.filter((blockedId) => dependencyNodeIds.has(blockedId)),
    blocks: task.blocks.filter((blockedId) => dependencyNodeIds.has(blockedId)),
    layer: resolveDependencyLayer(task.id),
  }));
  const graphLayers = Array.from(new Set(dependencyGraphNodesBase.map((node) => node.layer))).sort((a, b) => a - b);
  const graphWidth = 720;
  const columnGap = graphLayers.length > 1 ? (graphWidth - 160) / (graphLayers.length - 1) : 0;
  const dependencyGraphNodes: DependencyGraphNode[] = graphLayers.flatMap((layer, layerIndex) => {
    const nodes = dependencyGraphNodesBase
      .filter((node) => node.layer === layer)
      .sort((left, right) => {
        if (left.status !== right.status) {
          return left.status === 'open' ? -1 : 1;
        }
        const pressureDelta =
          right.blocks.length + right.blockedBy.length - (left.blocks.length + left.blockedBy.length);
        if (pressureDelta !== 0) {
          return pressureDelta;
        }
        return priorityRank(left.priority) - priorityRank(right.priority);
      });
    return nodes.map((node, nodeIndex) => {
      const verticalGap = 280 / Math.max(nodes.length, 1);
      const y = 68 + verticalGap * nodeIndex + verticalGap / 2;
      const x = 80 + columnGap * layerIndex;
      return { ...node, x, y };
    });
  });
  const dependencyGraphEdges: DependencyGraphEdge[] = dependencyGraphNodes.flatMap((node) =>
    node.blockedBy.map((blockedId) => ({ from: blockedId, to: node.id }))
  );
  const activeDependencyNodeId = hoveredDependencyNodeId ?? selectedTaskId ?? dependencyGraphNodes[0]?.id ?? null;
  const activeDependencyTask = activeDependencyNodeId ? taskMap.get(activeDependencyNodeId) ?? null : null;
  const activeDependencyLinks = activeDependencyTask
    ? {
        blockedBy: activeDependencyTask.blocked_by
          .map((taskId) => taskMap.get(taskId))
          .filter((task): task is Task => Boolean(task)),
        blocks: activeDependencyTask.blocks
          .map((taskId) => taskMap.get(taskId))
          .filter((task): task is Task => Boolean(task)),
      }
    : null;

  async function handleProjectPicker() {
    const folder = await openFolderDialog();
    if (folder) {
      await openProjectMutation.mutateAsync(folder);
    }
  }

  function updateComposer<K extends keyof ComposerState>(key: K, value: ComposerState[K]) {
    setComposer((current) => ({ ...current, [key]: value }));
  }

  async function handleComposerSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!composer.title.trim()) {
      setErrorMessage('Task title is required');
      return;
    }

    await createTaskMutation.mutateAsync(composer);
  }

  function resetQueueFilters() {
    setStatusFilter('all');
    setPriorityFilter('all');
    setSmartFilter('all');
    setDueFilter('all');
    setQueueEpicFilter('all');
    setQueueAssigneeFilter('all');
    setQueueTagFilter('all');
    setSelectedEpicId(null);
    setSearch('');
    setSortMode('priority');
  }

  function handleTaskFieldChange(taskId: number, updates: TaskUpdateInput) {
    updateTaskMutation.mutate({ id: taskId, updates });
  }

  function handleDetailDraftChange<K extends keyof DetailDraft>(key: K, value: DetailDraft[K]) {
    setDetailDraft((current) => ({ ...current, [key]: value }));
  }

  async function handleDetailDraftSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedTask) {
      return;
    }

    const trimmedTitle = detailDraft.title.trim();
    const trimmedDescription = detailDraft.description.trim();
    if (!trimmedTitle) {
      setErrorMessage('Task title is required');
      return;
    }

    await updateTaskMutation.mutateAsync({
      id: selectedTask.id,
      updates: {
        title: trimmedTitle,
        description: trimmedDescription,
        tags: detailDraft.tags.trim() || null,
      },
    });
  }

  function startPaneResize(kind: 'rail' | 'detail') {
    const container = kind === 'rail' ? shellRef.current : workspaceGridRef.current;
    if (!container) {
      return;
    }

    const bounds = container.getBoundingClientRect();
    const handleMove = (event: PointerEvent) => {
      if (kind === 'rail') {
        const nextWidth = Math.max(256, Math.min(460, event.clientX - bounds.left));
        setRailWidth(nextWidth);
        return;
      }
      const nextWidth = Math.max(300, Math.min(520, bounds.right - event.clientX));
      setDetailWidth(nextWidth);
    };

    const handleUp = () => {
      window.removeEventListener('pointermove', handleMove);
      window.removeEventListener('pointerup', handleUp);
    };

    window.addEventListener('pointermove', handleMove);
    window.addEventListener('pointerup', handleUp, { once: true });
  }

  return (
    <>
      <div className="app-shell" ref={shellRef} style={{ ['--rail-width' as string]: `${railWidth}px` }}>
      <aside className="project-rail">
        <div className="brand-block">
          <div className="brand-mark">
            <Sparkles size={18} />
          </div>
          <div>
            <p className="eyebrow">Native desktop</p>
            <h1>Mycelium</h1>
          </div>
        </div>

        <div className="project-card">
          <div className="project-card-head">
            <div>
              <p className="eyebrow">Current project</p>
              <strong>{projectPath ? projectPath.split('/').pop() : 'No folder selected'}</strong>
            </div>
            <button
              className="ghost-button"
              type="button"
              onClick={handleProjectPicker}
              disabled={openProjectMutation.isPending}
            >
              {openProjectMutation.isPending ? <Loader size={16} className="spin" /> : <FolderOpen size={16} />}
              <span>Open</span>
            </button>
          </div>
          <p className="project-path">{projectPath ?? 'Pick a project folder that already contains `.mycelium/mycelium.db`.'}</p>
        </div>

        <section className="rail-section">
          <div className="rail-section-head">
            <span>Epics</span>
            <Layers3 size={16} />
          </div>
          <div className="epic-stack">
            {epics.length === 0 ? (
              <div className="muted-panel">No epics yet. The app still works fine for task-only projects.</div>
            ) : (
              epicsWithStats.slice(0, 6).map((epic) => {
                const progress = epic.total_tasks > 0 ? Math.round(((epic.total_tasks - epic.open_tasks) / epic.total_tasks) * 100) : 0;
                return (
                  <button
                    key={epic.id}
                    className={`epic-chip ${selectedEpicId === epic.id ? 'is-active' : ''}`}
                    type="button"
                    onClick={() => setSelectedEpicId((current) => (current === epic.id ? null : epic.id))}
                  >
                    <div>
                      <strong>{epic.title}</strong>
                      <span>{epic.open_tasks} open</span>
                    </div>
                    <em>{progress}%</em>
                  </button>
                );
              })
            )}
          </div>
        </section>

        <section className="rail-section">
          <div className="rail-section-head">
            <span>Recent folders</span>
            <ChevronsUpDown size={16} />
          </div>
          <div className="recent-stack">
            {recentFolders.length === 0 ? (
              <div className="muted-panel">No recent projects yet.</div>
            ) : (
              recentFolders.map((folder) => (
                <button
                  key={folder}
                  className="recent-folder"
                  type="button"
                  onClick={() => openProjectMutation.mutate(folder)}
                >
                  <span>{folder.split('/').pop()}</span>
                  <small>{folder}</small>
                </button>
              ))
            )}
          </div>
        </section>
      </aside>
      <div
        className="pane-resizer pane-resizer-shell"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize project rail"
        onPointerDown={() => startPaneResize('rail')}
      />

      <main className="workspace">
        <header className="workspace-hero">
          <div>
            <p className="eyebrow">Overview</p>
            <h2>Project cockpit</h2>
            <p className="hero-copy">
              A calmer view of the work: compact queue in the middle, context on the right, and quick capture on demand.
            </p>
          </div>

          <div className="hero-actions">
            <div className="view-switch">
              <button
                className={`view-switch-button ${workspaceMode === 'overview' ? 'is-active' : ''}`}
                type="button"
                onClick={() => setWorkspaceMode('overview')}
              >
                <Layers3 size={16} />
                <span>Overview</span>
              </button>
              <button
                className={`view-switch-button ${workspaceMode === 'analytics' ? 'is-active' : ''}`}
                type="button"
                onClick={() => setWorkspaceMode('analytics')}
              >
                <BarChart3 size={16} />
                <span>Analytics</span>
              </button>
            </div>
            <label className="search-field">
              <Search size={16} />
              <input
                value={search}
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search titles, descriptions, people, tags"
              />
            </label>
            <button className="primary-button" type="button" onClick={() => setComposerOpen((current) => !current)}>
              {composerOpen ? <X size={16} /> : <Plus size={16} />}
              <span>{composerOpen ? 'Close composer' : 'New task'}</span>
            </button>
          </div>
        </header>

        <section className="summary-strip">
          {summaryCards.map((card) => (
            <article key={card.label} className={`summary-card accent-${card.accent}`}>
              <span>{card.label}</span>
              <strong>{card.value}</strong>
              <small>{card.hint}</small>
            </article>
          ))}
        </section>

        {errorMessage ? (
          <div className="banner error-banner">
            <AlertTriangle size={16} />
            <span>{errorMessage}</span>
          </div>
        ) : null}

        {workspaceMode === 'overview' ? (
        <section
          className="workspace-grid"
          ref={workspaceGridRef}
          style={{ ['--detail-width' as string]: `${detailWidth}px` }}
        >
          <div className="main-column">
            <div className="panel panel-list">
              <div className="panel-head">
                <div>
                  <p className="eyebrow">Queue</p>
                  <h3>Tasks in motion</h3>
                </div>
                <div className="task-count">
                  <CircleDot size={16} />
                  <span>{visibleTasks.length} shown</span>
                </div>
              </div>

              <div className="queue-toolbar">
                <div className="smart-filter-row">
                  {([
                    ['all', 'All'],
                    ['focus', 'Focus'],
                    ['up-next', 'Up next'],
                    ['unassigned', 'Unassigned'],
                    ['unscheduled', 'No due date'],
                    ['recent', 'Recently touched'],
                  ] as const).map(([value, label]) => (
                    <button
                      key={value}
                      className={`filter-chip ${smartFilter === value ? 'is-active' : ''}`}
                      type="button"
                      onClick={() => setSmartFilter(value)}
                    >
                      {label}
                    </button>
                  ))}
                </div>

                <div className="manual-filter-row">
                  <label className="filter-select">
                    <span>Epic</span>
                    <select value={queueEpicFilter} onChange={(event) => setQueueEpicFilter(event.target.value)}>
                      <option value="all">All epics</option>
                      {epics.map((epic) => (
                        <option key={epic.id} value={String(epic.id)}>
                          {epic.title}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="filter-select">
                    <span>Assignee</span>
                    <select value={queueAssigneeFilter} onChange={(event) => setQueueAssigneeFilter(event.target.value)}>
                      <option value="all">Everyone</option>
                      <option value="none">Unassigned</option>
                      {assignees.map((assignee) => (
                        <option key={assignee.id} value={String(assignee.id)}>
                          {assignee.name}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="filter-select">
                    <span>Tag</span>
                    <select value={queueTagFilter} onChange={(event) => setQueueTagFilter(event.target.value)}>
                      <option value="all">All tags</option>
                      {tags.map((tag) => (
                        <option key={tag} value={tag}>
                          {tag}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="filter-select">
                    <span>Due</span>
                    <select value={dueFilter} onChange={(event) => setDueFilter(event.target.value as DueFilter)}>
                      <option value="all">Any time</option>
                      <option value="overdue">Overdue</option>
                      <option value="today">Today</option>
                      <option value="soon">Next 3 days</option>
                      <option value="none">No due date</option>
                    </select>
                  </label>
                  <label className="filter-select">
                    <span>Sort</span>
                    <select value={sortMode} onChange={(event) => setSortMode(event.target.value as SortMode)}>
                      <option value="priority">Priority</option>
                      <option value="due">Due date</option>
                      <option value="updated">Last updated</option>
                      <option value="title">Title</option>
                    </select>
                  </label>
                </div>
              </div>

              <div className="filter-row queue-filter-row">
                <div className="chip-group">
                  {(['all', 'open', 'blocked', 'overdue', 'closed'] as StatusFilter[]).map((filter) => (
                    <button
                      key={filter}
                      className={`filter-chip ${statusFilter === filter ? 'is-active' : ''}`}
                      type="button"
                      onClick={() => setStatusFilter(filter)}
                    >
                      {filter}
                    </button>
                  ))}
                </div>

                <div className="chip-group queue-filter-actions">
                  {(['all', 'critical', 'high', 'medium', 'low'] as PriorityFilter[]).map((filter) => (
                    <button
                      key={filter}
                      className={`filter-chip ${priorityFilter === filter ? 'is-active' : ''}`}
                      type="button"
                      onClick={() => setPriorityFilter(filter)}
                    >
                      {filter}
                    </button>
                  ))}
                  <button className="ghost-button queue-clear-button" type="button" onClick={resetQueueFilters} disabled={activeFilterCount === 0}>
                    <X size={14} />
                    <span>Clear {activeFilterCount > 0 ? activeFilterCount : ''}</span>
                  </button>
                </div>
              </div>

              {loading ? (
                <div className="empty-state">
                  <Loader size={20} className="spin" />
                  <span>Loading project data…</span>
                </div>
              ) : visibleTasks.length === 0 ? (
                <div className="empty-state">
                  <Briefcase size={20} />
                  <span>No tasks match the current filters.</span>
                </div>
              ) : (
                <div className="task-list">
                  {visibleTasks.map((task) => {
                    const due = dueState(task);
                    const tag = firstTag(task.tags);
                    const selected = selectedTask?.id === task.id;

                    return (
                      <button
                        key={task.id}
                        type="button"
                        className={`task-row ${taskTone(task.priority)} ${selected ? 'is-selected' : ''}`}
                        onClick={() => startTransition(() => setSelectedTaskId(task.id))}
                      >
                        <div className="task-row-main">
                          <div className="task-row-titleline">
                            <span className={`priority-pill is-${task.priority}`}>
                              <span className="priority-pill-dot" />
                              {priorityLabel(task.priority)}
                            </span>
                            <strong>{task.title}</strong>
                            <span className={`status-pill ${task.status === 'closed' ? 'is-closed' : 'is-open'}`}>
                              {task.status}
                            </span>
                          </div>

                          <div className="task-row-meta">
                            <span>{task.epic_title || 'No epic'}</span>
                            <span>{task.assignee_name || 'Unassigned'}</span>
                            {tag ? <span>#{tag}</span> : null}
                            {task.blocked_by.length > 0 ? <span>Blocked by {task.blocked_by.length}</span> : null}
                            {task.blocks.length > 0 ? <span>Blocks {task.blocks.length}</span> : null}
                          </div>

                          <p className="task-card-preview">
                            {stripMarkdown(task.description) || 'No description yet.'}
                          </p>
                        </div>

                        <div className="task-row-side">
                          <span className="task-row-id">#{task.id}</span>
                          {due ? <span className={`due-pill ${due.tone}`}>{due.label}</span> : null}
                          <span className="task-row-updated">{formatDate(task.updated_at)}</span>
                        </div>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          </div>

          <div
            className="pane-resizer pane-resizer-workspace"
            role="separator"
            aria-orientation="vertical"
            aria-label="Resize detail panel"
            onPointerDown={() => startPaneResize('detail')}
          />
          <aside className="detail-column">
            <div className="panel detail-panel">
              <div className="panel-head">
                <div>
                  <p className="eyebrow">Selected task</p>
                  <h3>{selectedTask ? `#${selectedTask.id}` : 'Nothing selected'}</h3>
                </div>
                {selectedTask ? (
                  <button
                    className="primary-button"
                    type="button"
                    onClick={() => toggleTaskMutation.mutate(selectedTask)}
                    disabled={toggleTaskMutation.isPending}
                  >
                    {toggleTaskMutation.isPending ? (
                      <Loader size={16} className="spin" />
                    ) : selectedTask.status === 'closed' ? (
                      <ArrowRight size={16} />
                    ) : (
                      <CheckCheck size={16} />
                    )}
                    <span>{selectedTask.status === 'closed' ? 'Reopen' : 'Close task'}</span>
                  </button>
                ) : null}
              </div>

              {selectedTask ? (
                <>
                  <form className="detail-editor" onSubmit={handleDetailDraftSubmit}>
                    <label className="field field-span-2">
                      <span>Title</span>
                      <input
                        value={detailDraft.title}
                        onChange={(event) => handleDetailDraftChange('title', event.target.value)}
                        disabled={updateTaskMutation.isPending}
                      />
                    </label>
                    <label className="field field-span-2">
                      <span>Description</span>
                      <textarea
                        value={detailDraft.description}
                        onChange={(event) => handleDetailDraftChange('description', event.target.value)}
                        rows={6}
                        disabled={updateTaskMutation.isPending}
                      />
                    </label>
                    <label className="field field-span-2">
                      <span>Tags</span>
                      <input
                        value={detailDraft.tags}
                        onChange={(event) => handleDetailDraftChange('tags', event.target.value)}
                        placeholder="comma, separated, tags"
                        disabled={updateTaskMutation.isPending}
                      />
                    </label>
                    <div className="detail-editor-actions field-span-2">
                      <button className="primary-button" type="submit" disabled={updateTaskMutation.isPending}>
                        {updateTaskMutation.isPending ? <Loader size={16} className="spin" /> : null}
                        <span>Save content</span>
                      </button>
                    </div>
                  </form>

                  <div className="detail-title-block">
                    <MarkdownBlock content={selectedTask.description} />
                  </div>

                  <div className="detail-grid">
                    <label className="field detail-field">
                      <span>Status</span>
                      <select
                        value={selectedTask.status}
                        onChange={(event) =>
                          handleTaskFieldChange(selectedTask.id, {
                            status: event.target.value as Task['status'],
                          })
                        }
                        disabled={updateTaskMutation.isPending}
                      >
                        <option value="open">Open</option>
                        <option value="closed">Closed</option>
                      </select>
                    </label>
                    <label className="field detail-field">
                      <span>Priority</span>
                      <select
                        value={selectedTask.priority}
                        onChange={(event) =>
                          handleTaskFieldChange(selectedTask.id, {
                            priority: event.target.value as Task['priority'],
                          })
                        }
                        disabled={updateTaskMutation.isPending}
                      >
                        <option value="critical">Critical</option>
                        <option value="high">High</option>
                        <option value="medium">Medium</option>
                        <option value="low">Low</option>
                      </select>
                    </label>
                    <label className="field detail-field">
                      <span>Epic</span>
                      <select
                        value={selectedTask.epic_id ?? ''}
                        onChange={(event) =>
                          handleTaskFieldChange(selectedTask.id, {
                            epic_id: event.target.value ? Number(event.target.value) : null,
                          })
                        }
                        disabled={updateTaskMutation.isPending}
                      >
                        <option value="">No epic</option>
                        {epics.map((epic) => (
                          <option key={epic.id} value={epic.id}>
                            #{epic.id} {epic.title}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="field detail-field">
                      <span>Assignee</span>
                      <select
                        value={selectedTask.assignee_id ?? ''}
                        onChange={(event) =>
                          handleTaskFieldChange(selectedTask.id, {
                            assignee_id: event.target.value ? Number(event.target.value) : null,
                          })
                        }
                        disabled={updateTaskMutation.isPending}
                      >
                        <option value="">Unassigned</option>
                        {assignees.map((assignee) => (
                          <option key={assignee.id} value={assignee.id}>
                            {assignee.name}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label className="field detail-field">
                      <span>Due date</span>
                      <input
                        type="date"
                        value={selectedTask.due_date ?? ''}
                        onChange={(event) =>
                          handleTaskFieldChange(selectedTask.id, {
                            due_date: event.target.value || null,
                          })
                        }
                        disabled={updateTaskMutation.isPending}
                      />
                    </label>
                    <DetailStat label="Created" value={formatDate(selectedTask.created_at)} />
                  </div>

                  <div className="detail-strip">
                    <span>{selectedTask.tags ? selectedTask.tags : 'No tags'}</span>
                  </div>

                  <div className="mini-columns">
                    <MiniPanel
                      title="Ready soon"
                      items={highSignalTasks.map((task) => ({
                        id: task.id,
                        label: task.title,
                        meta: task.epic_title || priorityLabel(task.priority),
                        onClick: () => startTransition(() => setSelectedTaskId(task.id)),
                      }))}
                      emptyMessage="No high-signal tasks right now."
                    />
                    <MiniPanel
                      title="Urgent"
                      items={urgentTasks.map((task) => ({
                        id: task.id,
                        label: task.title,
                        meta: dueState(task)?.label || 'Open',
                        onClick: () => startTransition(() => setSelectedTaskId(task.id)),
                      }))}
                      emptyMessage="Nothing urgent."
                    />
                  </div>
                </>
              ) : (
                <div className="empty-state">
                  <Layers3 size={20} />
                  <span>Select a task to inspect its context.</span>
                </div>
              )}
            </div>

            <div className="panel">
              <div className="panel-head">
                <div>
                  <p className="eyebrow">Signal</p>
                  <h3>What matters now</h3>
                </div>
              </div>
              <div className="signal-stack">
                <SignalRow label="Open tasks" value={String(openTasks.length)} icon={<CircleDot size={16} />} />
                <SignalRow label="Blocked tasks" value={String(blockedTasks.length)} icon={<AlertTriangle size={16} />} />
                <SignalRow label="Assignees" value={String(assignees.length)} icon={<Briefcase size={16} />} />
                <SignalRow label="Epics" value={String(epics.length)} icon={<Layers3 size={16} />} />
                <SignalRow label="Tags" value={String(tags.length)} icon={<CalendarDays size={16} />} />
              </div>
            </div>
          </aside>
        </section>
        ) : (
        <section className="analytics-grid">
          <div className="panel analytics-panel analytics-panel-wide">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Window</p>
                <h3>Operational pulse</h3>
              </div>
              <div className="chip-group">
                {[7, 30, 90].map((windowSize) => (
                  <button
                    key={windowSize}
                    className={`filter-chip ${analyticsWindow === windowSize ? 'is-active' : ''}`}
                    type="button"
                    onClick={() => setAnalyticsWindow(windowSize as AnalyticsWindow)}
                  >
                    {windowSize}d
                  </button>
                ))}
              </div>
            </div>
            <div className="analytics-kpi-grid">
              {operationalSignals.map((signal) => (
                <article key={signal.label} className="analytics-kpi">
                  <span>{signal.label}</span>
                  <strong>{signal.value}</strong>
                  <small>{signal.meta}</small>
                </article>
              ))}
            </div>
          </div>

          <div className="panel analytics-panel">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Distribution</p>
                <h3>Priority mix</h3>
              </div>
            </div>
            <div className="priority-chart">
              {prioritySeries.map((entry) => (
                <div key={entry.label} className="priority-row">
                  <div className="priority-row-label">
                    <span className={`priority-dot is-${entry.priority}`} />
                    <strong>{entry.label}</strong>
                  </div>
                  <div className="priority-bar-track">
                    <div className={`priority-bar is-${entry.priority}`} style={{ width: `${(entry.value / maxPriorityValue) * 100}%` }} />
                  </div>
                  <span className="priority-value">{entry.value}</span>
                </div>
              ))}
            </div>
          </div>

          <div className="panel analytics-panel">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Progress</p>
                <h3>Completion ring</h3>
              </div>
            </div>
            <div className="donut-wrap">
              <div
                className="completion-donut"
                style={{ background: `conic-gradient(var(--sprout) 0 ${completionRate}%, rgba(255,255,255,0.07) ${completionRate}% 100%)` }}
              >
                <div className="completion-donut-core">
                  <strong>{completionRate}%</strong>
                  <span>done</span>
                </div>
              </div>
              <div className="donut-legend">
                <div><span className="legend-dot legend-open" />Open {openTasks.length}</div>
                <div><span className="legend-dot legend-closed" />Closed {closedTasks.length}</div>
                <div><span className="legend-dot legend-blocked" />Blocked {blockedTasks.length}</div>
              </div>
            </div>
          </div>

          <div className="panel analytics-panel analytics-panel-wide">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Epics</p>
                <h3>Epic health lanes</h3>
              </div>
            </div>
            <div className="epic-lanes">
              {analyticsEpics.length === 0 ? (
                <div className="muted-panel">No epics to chart yet.</div>
              ) : (
                analyticsEpics.map((epic) => {
                  const done = epic.total_tasks - epic.open_tasks;
                  return (
                    <div key={epic.id} className="epic-lane">
                      <div className="epic-lane-head">
                        <strong>{epic.title}</strong>
                        <span>{epic.open_tasks} open • {done}/{epic.total_tasks} closed</span>
                      </div>
                      <div className="epic-lane-track">
                        <div className="epic-lane-fill" style={{ width: `${epic.completion_pct}%` }} />
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </div>

          <div className="panel analytics-panel analytics-panel-wide">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Flow</p>
                <h3>Completion over time</h3>
              </div>
              <span className="panel-note">Created vs completed in rolling slices</span>
            </div>
            <div className="trend-chart">
              <svg viewBox="0 0 100 100" preserveAspectRatio="none" className="trend-svg" aria-hidden="true">
                <polyline className="trend-line trend-line-created" points={trendPointsCreated} />
                <polyline className="trend-line trend-line-completed" points={trendPointsCompleted} />
              </svg>
              <div className="trend-legend">
                <span><i className="legend-swatch legend-created" />Created</span>
                <span><i className="legend-swatch legend-completed" />Completed</span>
              </div>
              <div className="trend-labels">
                {completionTrend.map((entry) => (
                  <div key={entry.label} className="trend-label">
                    <strong>{entry.label}</strong>
                    <span>{entry.completed}/{entry.created}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className="panel analytics-panel analytics-panel-wide">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Timeline</p>
                <h3>Upcoming work</h3>
              </div>
            </div>
            <div className="timeline-list">
              {upcomingTasks.length === 0 ? (
                <div className="muted-panel">No scheduled due dates yet.</div>
              ) : (
                upcomingTasks.map((task) => (
                  <div key={task.id} className="timeline-item">
                    <div className={`timeline-marker is-${task.priority}`} />
                    <div className="timeline-copy">
                      <strong>{task.title}</strong>
                      <span>{task.epic_title || 'No epic'} • {formatDate(task.due_date)}</span>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>

          <div className="panel analytics-panel analytics-panel-wide">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Pressure</p>
                <h3>Assignee load</h3>
              </div>
            </div>
            {assigneeLoad.length === 0 ? (
              <div className="muted-panel">No assigned open work yet.</div>
            ) : (
              <div className="load-grid">
                {assigneeLoad.map((assignee) => (
                  <article key={assignee.id} className="load-card">
                    <div className="load-card-head">
                      <strong>{assignee.name}</strong>
                      <span>{assignee.open} open</span>
                    </div>
                    <div className="load-card-metrics">
                      <span>{assignee.critical} high signal</span>
                      <span>{assignee.blocked} blocked</span>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </div>

          <div className="panel analytics-panel analytics-panel-wide">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Aging</p>
                <h3>Open work age</h3>
              </div>
            </div>
            <div className="aging-grid">
              {agingBuckets.map((bucket) => (
                <article key={bucket.label} className="aging-card">
                  <div className="aging-card-head">
                    <span>{bucket.label}</span>
                    <span className={`priority-chip ${bucket.tone}`}>{bucket.value}</span>
                  </div>
                  <div className="priority-bar-track">
                    <div
                      className={`priority-bar ${bucket.tone}`}
                      style={{ width: `${(bucket.value / Math.max(1, ...agingBuckets.map((entry) => entry.value))) * 100}%` }}
                    />
                  </div>
                </article>
              ))}
            </div>
          </div>

          <div className="panel analytics-panel analytics-panel-wide">
            <div className="panel-head">
              <div>
                <p className="eyebrow">Dependencies</p>
                <h3>Blocker network</h3>
              </div>
              <span className="panel-note">Hotspots surface tasks with the most dependency pressure</span>
            </div>
            <div className="dependency-graph-wrap">
              {dependencyGraphNodes.length === 0 ? (
                <div className="muted-panel">No dependency edges to visualize yet.</div>
              ) : (
                <div className="dependency-graph-layout">
                  <svg viewBox="0 0 720 340" className="dependency-graph" role="img" aria-label="Task dependency graph">
                    <defs>
                      <marker
                        id="dependency-arrow"
                        viewBox="0 0 10 10"
                        refX="8"
                        refY="5"
                        markerWidth="6"
                        markerHeight="6"
                        orient="auto-start-reverse"
                      >
                        <path d="M 0 0 L 10 5 L 0 10 z" fill="rgba(226, 230, 227, 0.6)" />
                      </marker>
                    </defs>
                    {graphLayers.map((layer, index) => {
                      const labelX = 80 + columnGap * index;
                      return (
                        <g key={layer}>
                          <text x={labelX} y="18" textAnchor="middle" className="dependency-layer-label">
                            {index === 0 ? 'Roots' : `Layer ${index + 1}`}
                          </text>
                          <line
                            x1={labelX}
                            x2={labelX}
                            y1="28"
                            y2="326"
                            className="dependency-layer-line"
                          />
                        </g>
                      );
                    })}
                    {dependencyGraphEdges.map((edge) => {
                      const from = dependencyGraphNodes.find((node) => node.id === edge.from);
                      const to = dependencyGraphNodes.find((node) => node.id === edge.to);
                      if (!from || !to) return null;
                      const isActive = activeDependencyNodeId === edge.from || activeDependencyNodeId === edge.to;

                      return (
                        <path
                          key={`${edge.from}-${edge.to}`}
                          className={`dependency-edge ${isActive ? 'is-active' : ''}`}
                          markerEnd="url(#dependency-arrow)"
                          d={`M ${from.x + 34} ${from.y} C ${(from.x + to.x) / 2} ${from.y}, ${(from.x + to.x) / 2} ${to.y}, ${to.x - 34} ${to.y}`}
                        />
                      );
                    })}
                    {dependencyGraphNodes.map((node) => {
                      const isConnected =
                        activeDependencyTask?.id === node.id ||
                        activeDependencyTask?.blocked_by.includes(node.id) ||
                        activeDependencyTask?.blocks.includes(node.id);
                      return (
                        <g
                          key={node.id}
                          className={`dependency-node is-${node.priority} ${selectedTaskId === node.id ? 'is-selected' : ''} ${hoveredDependencyNodeId === node.id ? 'is-hovered' : ''} ${isConnected ? 'is-connected' : ''}`}
                          transform={`translate(${node.x}, ${node.y})`}
                          onMouseEnter={() => setHoveredDependencyNodeId(node.id)}
                          onMouseLeave={() => setHoveredDependencyNodeId((current) => (current === node.id ? null : current))}
                          onClick={() => {
                            setWorkspaceMode('overview');
                            startTransition(() => setSelectedTaskId(node.id));
                          }}
                        >
                          <rect x="-34" y="-24" width="68" height="48" rx="18" />
                          <text textAnchor="middle" y="-5">#{node.id}</text>
                          <text textAnchor="middle" y="9" className="dependency-node-title">
                            {node.title.length > 16 ? `${node.title.slice(0, 16)}…` : node.title}
                          </text>
                          <text textAnchor="middle" y="22" className="dependency-node-subtext">
                            {node.blocks.length} out • {node.blockedBy.length} in
                          </text>
                        </g>
                      );
                    })}
                  </svg>
                  <div className="dependency-spotlight">
                    {activeDependencyTask ? (
                      <>
                        <div className="dependency-spotlight-head">
                          <div>
                            <p className="eyebrow">Focused task</p>
                            <h4>#{activeDependencyTask.id} {activeDependencyTask.title}</h4>
                          </div>
                          <span className={`priority-chip is-${activeDependencyTask.priority}`}>
                            {priorityLabel(activeDependencyTask.priority)}
                          </span>
                        </div>
                        <p className="dependency-spotlight-copy">
                          {stripMarkdown(activeDependencyTask.description) || 'No description yet.'}
                        </p>
                        <div className="dependency-spotlight-meta">
                          <DetailStat label="Epic" value={activeDependencyTask.epic_title || 'No epic'} />
                          <DetailStat label="Status" value={activeDependencyTask.status} />
                          <DetailStat
                            label="Due"
                            value={activeDependencyTask.due_date ? formatDate(activeDependencyTask.due_date) : 'Unset'}
                          />
                        </div>
                        <div className="dependency-link-columns">
                          <MiniPanel
                            title="Blocked by"
                            emptyMessage="No blockers in scope."
                            items={(activeDependencyLinks?.blockedBy ?? []).slice(0, 4).map((task) => ({
                              label: `#${task.id} ${task.title}`,
                              meta: task.status,
                              onClick: () => setHoveredDependencyNodeId(task.id),
                            }))}
                          />
                          <MiniPanel
                            title="Blocking"
                            emptyMessage="Not blocking other tasks in scope."
                            items={(activeDependencyLinks?.blocks ?? []).slice(0, 4).map((task) => ({
                              label: `#${task.id} ${task.title}`,
                              meta: task.status,
                              onClick: () => setHoveredDependencyNodeId(task.id),
                            }))}
                          />
                        </div>
                      </>
                    ) : (
                      <div className="muted-panel">Hover or select a node to inspect its dependency context.</div>
                    )}
                  </div>
                </div>
              )}
            </div>
            {dependencyGraphNodes.length > 0 ? (
              <div className="dependency-caption">
                <span>Hover to trace adjacent edges. Click a node to jump back to the task in Overview.</span>
                <span>Flow runs left to right from blockers to blocked work.</span>
              </div>
            ) : null}
          </div>
        </section>
        )}
      </main>
      </div>
      {composerOpen ? (
        <div className="modal-backdrop" role="presentation" onClick={() => setComposerOpen(false)}>
          <div
            className="modal-card"
            role="dialog"
            aria-modal="true"
            aria-label="Create task"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="panel-head modal-head">
              <div>
                <p className="eyebrow">Capture</p>
                <h3>Quick composer</h3>
              </div>
              <button className="ghost-button" type="button" onClick={() => setComposerOpen(false)}>
                <X size={16} />
                <span>Close</span>
              </button>
            </div>

            <form className="composer-grid modal-composer-grid" onSubmit={handleComposerSubmit}>
              <label className="field field-span-2">
                <span>Title</span>
                <input
                  ref={composerTitleRef}
                  value={composer.title}
                  onChange={(event) => updateComposer('title', event.target.value)}
                  placeholder="Ship a cleaner task dashboard"
                />
              </label>
              <label className="field field-span-2">
                <span>Description</span>
                <textarea
                  value={composer.description}
                  onChange={(event) => updateComposer('description', event.target.value)}
                  placeholder="Add the useful context the future reader will need."
                  rows={4}
                />
              </label>
              <label className="field">
                <span>Epic</span>
                <select value={composer.epicId} onChange={(event) => updateComposer('epicId', event.target.value)}>
                  <option value="">No epic</option>
                  {epicsWithStats.map((epic) => (
                    <option key={epic.id} value={String(epic.id)}>
                      {epic.title}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>Priority</span>
                <select
                  value={composer.priority}
                  onChange={(event) => updateComposer('priority', event.target.value as Priority)}
                >
                  <option value="low">Low</option>
                  <option value="medium">Medium</option>
                  <option value="high">High</option>
                  <option value="critical">Critical</option>
                </select>
              </label>
              <label className="field">
                <span>Assignee</span>
                <select
                  value={composer.assigneeId}
                  onChange={(event) => updateComposer('assigneeId', event.target.value)}
                >
                  <option value="">Unassigned</option>
                  {assignees.map((assignee) => (
                    <option key={assignee.id} value={String(assignee.id)}>
                      {assignee.name}
                    </option>
                  ))}
                </select>
              </label>
              <label className="field">
                <span>Due date</span>
                <input
                  type="date"
                  value={composer.dueDate}
                  onChange={(event) => updateComposer('dueDate', event.target.value)}
                />
              </label>
              <label className="field field-span-2">
                <span>Tags</span>
                <input
                  value={composer.tags}
                  onChange={(event) => updateComposer('tags', event.target.value)}
                  placeholder={tags.length > 0 ? `Try: ${tags.slice(0, 4).join(', ')}` : 'frontend, polish, release'}
                />
              </label>
              <div className="composer-actions field-span-2">
                <button className="ghost-button" type="button" onClick={() => setComposer(initialComposerState)}>
                  Reset
                </button>
                <button className="primary-button" type="submit" disabled={createTaskMutation.isPending}>
                  {createTaskMutation.isPending ? <Loader size={16} className="spin" /> : <Plus size={16} />}
                  <span>Create task</span>
                </button>
              </div>
            </form>
          </div>
        </div>
      ) : null}
    </>
  );
}

function DetailStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="detail-stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function MiniPanel({
  title,
  items,
  emptyMessage,
}: {
  title: string;
  items: MiniPanelItem[];
  emptyMessage: string;
}) {
  return (
    <div className="mini-panel">
      <div className="mini-panel-head">
        <span>{title}</span>
      </div>
      {items.length === 0 ? (
        <p className="mini-empty">{emptyMessage}</p>
      ) : (
        items.map((item, index) => (
          <button
            key={item.id ?? `${item.label}-${index}`}
            type="button"
            className={`mini-task ${item.onClick ? 'is-clickable' : ''}`}
            onClick={item.onClick}
            disabled={!item.onClick}
          >
            <strong>{item.label}</strong>
            <span>{item.meta}</span>
          </button>
        ))
      )}
    </div>
  );
}

function SignalRow({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon: React.ReactNode;
}) {
  return (
    <div className="signal-row">
      <div>
        {icon}
        <span>{label}</span>
      </div>
      <strong>{value}</strong>
    </div>
  );
}

export default App;
