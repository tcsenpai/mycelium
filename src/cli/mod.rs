use clap::{Args, Parser, Subcommand};

pub mod id_parser;

#[derive(Parser)]
#[command(name = "myc")]
#[command(about = "A robust, production-grade task/plan manager CLI")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output format (table, json)
    #[arg(global = true, short, long, default_value = "table")]
    pub format: OutputFormat,

    /// Suppress non-error output
    #[arg(global = true, short, long)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new mycelium project
    Init {
        /// Skip installing the follow-up Stop hook into .claude/
        #[arg(long)]
        no_hooks: bool,

        /// Create a project here without prompting, even if a parent directory
        /// already has one (skips the nested-project confirmation).
        #[arg(long)]
        force: bool,
    },

    /// Generate or update AGENTS.md with mycelium instructions
    PrimeAgents {
        /// Force regeneration even if file exists
        #[arg(long)]
        force: bool,

        /// Path to AGENTS.md (defaults to ./AGENTS.md)
        #[arg(long)]
        path: Option<std::path::PathBuf>,
    },

    /// Manage epics
    #[command(subcommand)]
    Epic(EpicCommands),

    /// Manage tasks
    #[command(subcommand)]
    Task(TaskCommands),

    /// Manage assignees
    #[command(subcommand)]
    Assignee(AssigneeCommands),

    /// Manage dependencies
    #[command(subcommand)]
    Deps(DepsCommands),

    /// List and filter tasks
    List(ListArgs),

    /// Show project summary
    Summary,

    /// Export data
    #[command(subcommand)]
    Export(ExportCommands),

    /// Run health checks on the project
    Doctor {
        /// Automatically fix issues where possible
        #[arg(long)]
        fix: bool,
    },

    /// Linear integration (bidirectional sync)
    #[command(subcommand)]
    Linear(LinearCommands),

    /// Manage follow-ups (lightweight "oh-by-the-way" items)
    #[command(subcommand, alias = "fu")]
    Followup(FollowupCommands),

    /// Manage the Claude Code follow-up Stop hook
    #[command(subcommand)]
    Hooks(HooksCommands),

    /// Update the myc binary (via cargo) and resync AGENTS.md + the hook
    Update,

    /// Show myc and mycelium-core versions (supports --format json)
    Version,
}

#[derive(Subcommand)]
pub enum HooksCommands {
    /// Install the follow-up hook (project-local by default)
    Install {
        /// Install into ~/.claude instead of the project's .claude/
        #[arg(long)]
        global: bool,
    },

    /// Remove the follow-up hook
    Uninstall {
        /// Remove from ~/.claude instead of the project's .claude/
        #[arg(long)]
        global: bool,
    },

    /// Show where the follow-up hook is installed
    Status,
}

#[derive(Subcommand)]
pub enum FollowupCommands {
    /// Add a follow-up (body required, title optional)
    Add {
        /// Follow-up body (required)
        body: String,

        /// Optional title (defaults to first 60 chars of body)
        #[arg(short, long)]
        title: Option<String>,
    },

    /// List follow-ups (defaults to all)
    List {
        /// Filter by exact status: open, in_progress, done, wontfix
        #[arg(short, long, conflicts_with_all = ["all", "open", "closed"])]
        status: Option<String>,

        /// Show all follow-ups (default)
        #[arg(short = 'a', long, conflicts_with_all = ["open", "closed"])]
        all: bool,

        /// Show only active items (open + in_progress)
        #[arg(short = 'o', long, conflicts_with = "closed")]
        open: bool,

        /// Show only closed items (done + wontfix)
        #[arg(short = 'c', long)]
        closed: bool,
    },

    /// Show a follow-up
    Show {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,
    },

    /// Pull the lowest-ID active follow-up (agent loop primitive)
    Next,

    /// Mark a follow-up as in_progress
    Start {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,
    },

    /// Mark a follow-up as done
    Done {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,

        /// Optional closure reason
        #[arg(short, long)]
        reason: Option<String>,
    },

    /// Mark a follow-up as wontfix
    Wontfix {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,

        /// Optional closure reason
        #[arg(short, long)]
        reason: Option<String>,
    },

    /// Reopen a follow-up
    Reopen {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,
    },

    /// Replace body or title of a follow-up
    Edit {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,

        /// New body (omit to keep current)
        #[arg(short, long)]
        body: Option<String>,

        /// New title (use `-` to clear, omit to keep current)
        #[arg(short, long)]
        title: Option<String>,
    },

    /// Append text to a follow-up's body (timestamped, preserves existing)
    Append {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,

        /// Text to append
        text: String,
    },

    /// Delete a follow-up
    Rm {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,

        /// Skip confirmation
        #[arg(long)]
        force: bool,
    },

    /// Promote a follow-up to a full task (marks follow-up done)
    Promote {
        /// Follow-up ID
        #[arg(value_parser = id_parser::followup_id)]
        id: i64,

        /// Epic ID to assign the new task to
        #[arg(short, long, value_parser = id_parser::epic_id)]
        epic: Option<i64>,

        /// Priority (low, medium, high, critical)
        #[arg(short, long, default_value = "medium")]
        priority: String,
    },

    /// Show counts (great for end-of-task agent checks)
    Count,

    /// Silence the end-of-task follow-up Stop-hook for the next N stops
    /// (project-scoped). Use after you've already surfaced follow-ups so the
    /// hook stops re-nagging every turn.
    Snooze {
        /// Number of stops to stay silent (0 clears an active snooze)
        #[arg(short, long, default_value = "5")]
        turns: u32,
    },
}

#[derive(Subcommand)]
pub enum LinearCommands {
    /// Set up Linear integration (API key, team, mappings)
    Setup,

    /// Bidirectional sync with Linear
    Sync {
        /// Force local changes to win on conflicts
        #[arg(long)]
        force_local: bool,

        /// Force remote changes to win on conflicts
        #[arg(long)]
        force_remote: bool,
    },

    /// Push local tasks to Linear
    Push,

    /// Pull Linear issues to local
    Pull,

    /// Show Linear integration status
    Status,

    /// Remove Linear integration and sync data
    Unlink,
}

#[derive(Subcommand)]
pub enum EpicCommands {
    /// Create a new epic
    Create {
        /// Epic title
        #[arg(short, long)]
        title: String,

        /// Epic description
        #[arg(short, long)]
        description: Option<String>,

        /// Notes (free-form text for comments, tips, or context)
        #[arg(short = 'n', long)]
        notes: Option<String>,

        /// Additional user info (context for agents or collaborators)
        #[arg(long)]
        user_info: Option<String>,
    },

    /// List all epics
    List,

    /// Show epic details
    Show {
        /// Epic ID
        #[arg(value_parser = id_parser::epic_id)]
        id: i64,
    },

    /// Update an epic
    Update {
        /// Epic ID
        #[arg(value_parser = id_parser::epic_id)]
        id: i64,

        /// New title
        #[arg(short, long)]
        title: Option<String>,

        /// New description
        #[arg(short, long)]
        description: Option<String>,

        /// New status
        #[arg(short, long, value_parser = ["open", "in_progress", "closed"])]
        status: Option<String>,

        /// New notes (use - to remove)
        #[arg(short = 'n', long)]
        notes: Option<String>,

        /// New user info (use - to remove)
        #[arg(long)]
        user_info: Option<String>,

        /// Agent questions (use - to remove)
        #[arg(long)]
        agent_questions: Option<String>,
    },

    /// Add a note to an epic
    Note {
        /// Epic ID
        #[arg(value_parser = id_parser::epic_id)]
        epic_id: i64,

        /// Note content
        content: String,
    },

    /// Show epic notes
    Notes {
        /// Epic ID
        #[arg(value_parser = id_parser::epic_id)]
        epic_id: i64,
    },

    /// Delete an epic
    Delete {
        /// Epic ID
        #[arg(value_parser = id_parser::epic_id)]
        id: i64,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum TaskCommands {
    /// Create a new task
    Create {
        /// Task title (positional). Alternative to --title; give exactly one.
        #[arg(value_name = "TITLE", conflicts_with = "title")]
        title_pos: Option<String>,

        /// Task title (flag form). Alternative to the positional TITLE.
        #[arg(short, long)]
        title: Option<String>,

        /// Task description
        #[arg(short, long)]
        description: Option<String>,

        /// Epic ID to assign to
        #[arg(short, long, value_parser = id_parser::epic_id)]
        epic: Option<i64>,

        /// Parent task ID (makes this a subtask/child of that task)
        #[arg(long, value_parser = id_parser::task_id)]
        parent: Option<i64>,

        /// Priority (low, medium, high, critical)
        #[arg(short, long, default_value = "medium")]
        priority: String,

        /// Assignee ID
        #[arg(short, long, value_parser = id_parser::assignee_id)]
        assignee: Option<i64>,

        /// Due date (YYYY-MM-DD)
        #[arg(short = 'u', long)]
        due: Option<String>,

        /// Tags (comma-separated, e.g., "frontend,urgent")
        #[arg(short = 'g', long)]
        tags: Option<String>,

        /// Notes (free-form text for comments, tips, or context)
        #[arg(short = 'n', long)]
        notes: Option<String>,

        /// Additional user info (context for agents or collaborators)
        #[arg(long)]
        user_info: Option<String>,

        /// Use a template
        #[arg(short = 'm', long)]
        template: Option<String>,
    },

    /// List tasks
    List {
        /// Filter by epic ID
        #[arg(short, long, value_parser = id_parser::epic_id)]
        epic: Option<i64>,

        /// Filter by status (defaults to 'open')
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by priority
        #[arg(short, long)]
        priority: Option<String>,

        /// Filter by assignee ID
        #[arg(short, long, value_parser = id_parser::assignee_id)]
        assignee: Option<i64>,

        /// Show only blocked tasks
        #[arg(long)]
        blocked: bool,

        /// Show only overdue tasks
        #[arg(long)]
        overdue: bool,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Show all tasks including closed (overrides default open filter)
        #[arg(long)]
        all: bool,

        /// Render as a parent>child tree. A child that matches the filters is
        /// shown nested under its parent even when the parent itself does not
        /// match (parent shown as dimmed context, not as an orphan).
        #[arg(long)]
        tree: bool,

        /// Filter to the direct children of this task ID (0 = only top-level
        /// tasks with no parent). Combine with --tag/--status/etc.
        #[arg(long, value_parser = id_parser::task_id)]
        parent: Option<i64>,
    },

    /// Batch create tasks from JSON file
    Batch {
        /// JSON file path
        #[arg(short, long)]
        file: String,
    },

    /// Show task details
    Show {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        id: i64,
    },

    /// Update a task
    Update {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        id: i64,

        /// New title
        #[arg(short, long)]
        title: Option<String>,

        /// New description
        #[arg(short, long)]
        description: Option<String>,

        /// New status
        #[arg(short, long)]
        status: Option<String>,

        /// New priority
        #[arg(short, long)]
        priority: Option<String>,

        /// New epic ID (use 0 to remove)
        #[arg(short, long, value_parser = id_parser::epic_id)]
        epic: Option<i64>,

        /// New parent task ID (use 0 to detach / make top-level)
        #[arg(long, value_parser = id_parser::task_id)]
        parent: Option<i64>,

        /// New assignee ID (use 0 to remove)
        #[arg(short, long, value_parser = id_parser::assignee_id)]
        assignee: Option<i64>,

        /// New due date (YYYY-MM-DD)
        #[arg(short = 'u', long)]
        due: Option<String>,

        /// New tags (comma-separated, use - to remove)
        #[arg(short = 'g', long)]
        tags: Option<String>,

        /// New notes (use - to remove)
        #[arg(short = 'n', long)]
        notes: Option<String>,

        /// New user info (use - to remove)
        #[arg(long)]
        user_info: Option<String>,

        /// Agent questions (use - to remove)
        #[arg(long)]
        agent_questions: Option<String>,
    },

    /// Delete a task
    Delete {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        id: i64,

        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Assign a task to someone
    Assign {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        task_id: i64,

        /// Assignee ID (use 0 to unassign)
        #[arg(value_parser = id_parser::assignee_id)]
        assignee_id: i64,
    },

    /// Link task to external resources
    #[command(subcommand)]
    Link(LinkCommands),

    /// Unlink external reference
    Unlink {
        /// Reference ID
        #[arg(value_parser = id_parser::ref_id)]
        ref_id: i64,
    },

    /// Show non-blocking references (relates/duplicate) for a task
    Refs {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        id: i64,
    },

    /// Remove a non-blocking reference between two tasks
    RefUnlink {
        /// First task ID
        #[arg(value_parser = id_parser::task_id)]
        task: i64,

        /// Second task ID
        #[arg(value_parser = id_parser::task_id)]
        other: i64,

        /// Reference type to remove (relates or duplicate)
        #[arg(value_parser = ["relates", "duplicate"])]
        ref_type: String,
    },

    /// Close a task
    Close {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        id: i64,

        /// Force close even if blocked
        #[arg(long)]
        force: bool,

        /// Also close all open child subtasks (default: prompt when the task
        /// has open children; children are never closed without this or a yes)
        #[arg(long)]
        cascade: bool,
    },

    /// Reopen a task
    Reopen {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        id: i64,
    },

    /// Add a note to a task
    Note {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        task_id: i64,

        /// Note content
        content: String,
    },

    /// Show task notes
    Notes {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        task_id: i64,
    },

    /// Clone a task
    Clone {
        /// Task ID to clone
        #[arg(value_parser = id_parser::task_id)]
        id: i64,

        /// New title (optional, defaults to "Original (copy)")
        #[arg(short, long)]
        title: Option<String>,
    },

    /// Batch operations on multiple tasks
    #[command(subcommand)]
    BatchOp(BatchOpCommands),
}

#[derive(Subcommand)]
pub enum BatchOpCommands {
    /// Close multiple tasks
    Close {
        /// Task IDs to close
        #[arg(value_parser = id_parser::task_id)]
        ids: Vec<i64>,

        /// Force close even if blocked
        #[arg(long)]
        force: bool,
    },

    /// Add a tag to multiple tasks
    Tag {
        /// Tag to add
        tag: String,

        /// Task IDs
        #[arg(value_parser = id_parser::task_id)]
        ids: Vec<i64>,
    },

    /// Move multiple tasks to an epic
    Move {
        /// Epic ID (use 0 for no epic)
        #[arg(value_parser = id_parser::epic_id)]
        epic_id: i64,

        /// Task IDs
        #[arg(value_parser = id_parser::task_id)]
        ids: Vec<i64>,
    },

    /// Delete tasks that are not assigned to any epic
    DeleteOrphans {
        /// Force deletion without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum LinkCommands {
    /// Link to GitHub issue
    GithubIssue {
        /// Task ID
        #[arg(short, long, value_parser = id_parser::task_id)]
        task: i64,

        /// GitHub reference (owner/repo#number)
        reference: String,
    },

    /// Link to GitHub PR
    GithubPr {
        /// Task ID
        #[arg(short, long, value_parser = id_parser::task_id)]
        task: i64,

        /// GitHub reference (owner/repo#number)
        reference: String,
    },

    /// Link to URL
    Url {
        /// Task ID
        #[arg(short, long, value_parser = id_parser::task_id)]
        task: i64,

        /// URL
        url: String,
    },

    /// Mark task as blocking another
    Blocks {
        /// Task ID that blocks
        #[arg(short, long, value_parser = id_parser::task_id)]
        task: i64,

        /// Task ID being blocked
        #[arg(value_parser = id_parser::task_id)]
        blocked: i64,
    },

    /// Mark two tasks as related (non-blocking, symmetric)
    Relates {
        /// First task ID
        #[arg(value_parser = id_parser::task_id)]
        task: i64,

        /// Second task ID
        #[arg(value_parser = id_parser::task_id)]
        other: i64,
    },

    /// Mark two tasks as duplicates (non-blocking, symmetric; marks only,
    /// never closes or merges anything)
    Duplicate {
        /// First task ID
        #[arg(value_parser = id_parser::task_id)]
        task: i64,

        /// Second task ID
        #[arg(value_parser = id_parser::task_id)]
        other: i64,
    },
}

#[derive(Subcommand)]
pub enum AssigneeCommands {
    /// Create a new assignee
    Create {
        /// Assignee name
        #[arg(short, long)]
        name: String,

        /// Email address
        #[arg(short, long)]
        email: Option<String>,

        /// GitHub username
        #[arg(short, long)]
        github: Option<String>,
    },

    /// List all assignees
    List,

    /// Show assignee details
    Show {
        /// Assignee ID
        #[arg(value_parser = id_parser::assignee_id)]
        id: i64,
    },

    /// Delete an assignee
    Delete {
        /// Assignee ID
        #[arg(value_parser = id_parser::assignee_id)]
        id: i64,

        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum DepsCommands {
    /// Show dependency tree for a task
    Show {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        task_id: i64,
    },

    /// Remove a dependency
    Unlink {
        /// Task ID
        #[arg(value_parser = id_parser::task_id)]
        task_id: i64,

        /// Task ID that was being blocked
        #[arg(value_parser = id_parser::task_id)]
        blocked_task_id: i64,
    },
}

#[derive(Args)]
pub struct ListArgs {
    /// Filter by epic ID
    #[arg(short, long, value_parser = id_parser::epic_id)]
    pub epic: Option<i64>,

    /// Filter by status (defaults to 'open')
    #[arg(short, long)]
    pub status: Option<String>,

    /// Filter by priority
    #[arg(short, long)]
    pub priority: Option<String>,

    /// Filter by assignee ID
    #[arg(short, long, value_parser = id_parser::assignee_id)]
    pub assignee: Option<i64>,

    /// Show only blocked tasks
    #[arg(long)]
    pub blocked: bool,

    /// Show only overdue tasks
    #[arg(long)]
    pub overdue: bool,

    /// Filter by tag
    #[arg(short, long)]
    pub tag: Option<String>,

    /// Show all tasks including closed (overrides default open filter)
    #[arg(long)]
    pub all: bool,

    /// Render as a parent>child tree (see `myc task list --tree`)
    #[arg(long)]
    pub tree: bool,

    /// Filter to the direct children of this task ID (0 = only top-level)
    #[arg(long, value_parser = id_parser::task_id)]
    pub parent: Option<i64>,
}

#[derive(Subcommand)]
pub enum ExportCommands {
    /// Export to JSON
    Json {
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Export to CSV
    Csv {
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub enum OutputFormat {
    Table,
    Json,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            _ => Err(format!("Invalid format: {}. Use 'table' or 'json'", s)),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
        }
    }
}
