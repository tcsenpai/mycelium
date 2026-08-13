use clap::Parser;

mod cli;
mod commands;
mod linear;

// Data layer now lives in the shared `mycelium-core` crate. Re-export the
// modules under the old paths so the CLI code (crate::db / crate::models /
// crate::error) keeps working unchanged.
pub use mycelium_core::{db, error, models};

use cli::{
    AssigneeCommands, BatchOpCommands, Cli, Commands, DepsCommands, EpicCommands, ExportCommands,
    FollowupCommands, HooksCommands, LinearCommands, LinkCommands, TaskCommands,
};

pub use commands::{ERROR_PREFIX, INFO_PREFIX, SUCCESS_PREFIX, WARNING_PREFIX};

/// Map the `--global` flag to a hook install scope.
fn scope(global: bool) -> commands::hooks::Scope {
    if global {
        commands::hooks::Scope::Global
    } else {
        commands::hooks::Scope::Local
    }
}

/// Print an error and exit. Lives in the CLI binary (not core) because it does
/// terminal I/O and process exit.
fn handle_error(err: error::MyceliumError) -> ! {
    eprintln!("{} {}", ERROR_PREFIX, err);
    std::process::exit(1);
}

fn main() {
    let cli = Cli::parse();

    // First run in this repo after a myc upgrade: reconcile AGENTS.md and the
    // Stop hook. Self-gating and best-effort — never blocks the real command.
    commands::autorefresh::maybe_refresh(&cli.command);

    let result = match cli.command {
        Commands::Init { no_hooks } => commands::init::execute(false, no_hooks),

        Commands::PrimeAgents { force, path } => {
            commands::init::execute_prime_agents(force, path.as_deref())
        }

        Commands::Epic(cmd) => match cmd {
            EpicCommands::Create {
                title,
                description,
                notes,
                user_info,
            } => commands::epic::create(
                &title,
                description.as_deref(),
                notes.as_deref(),
                user_info.as_deref(),
                &cli.format,
                cli.quiet,
            ),
            EpicCommands::List => commands::epic::list(&cli.format, cli.quiet),
            EpicCommands::Show { id } => commands::epic::show(id, &cli.format, cli.quiet),
            EpicCommands::Update {
                id,
                title,
                description,
                status,
                notes,
                user_info,
                agent_questions,
            } => commands::epic::update(
                id,
                title.as_deref(),
                description.as_deref(),
                status.as_deref(),
                notes.as_deref(),
                user_info.as_deref(),
                agent_questions.as_deref(),
                &cli.format,
                cli.quiet,
            ),
            EpicCommands::Note { epic_id, content } => {
                commands::epic::add_note(epic_id, &content, &cli.format, cli.quiet)
            }
            EpicCommands::Notes { epic_id } => {
                commands::epic::show_notes(epic_id, &cli.format, cli.quiet)
            }
            EpicCommands::Delete { id, force } => commands::epic::delete(id, force, cli.quiet),
        },

        Commands::Task(cmd) => match cmd {
            TaskCommands::Create {
                title,
                description,
                epic,
                priority,
                assignee,
                due,
                tags,
                notes,
                user_info,
                template,
            } => commands::task::create(
                &title,
                description.as_deref(),
                epic,
                &priority,
                assignee,
                due.as_deref(),
                tags.as_deref(),
                notes.as_deref(),
                user_info.as_deref(),
                template.as_deref(),
                &cli.format,
                cli.quiet,
            ),
            TaskCommands::List {
                epic,
                status,
                priority,
                assignee,
                blocked,
                overdue,
                tag,
                all,
            } => commands::task::list(
                epic,
                status.as_deref(),
                priority.as_deref(),
                assignee,
                blocked,
                overdue,
                tag.as_deref(),
                all,
                &cli.format,
                cli.quiet,
            ),
            TaskCommands::Batch { file } => commands::task::batch(&file, &cli.format, cli.quiet),
            TaskCommands::Show { id } => commands::task::show(id, &cli.format, cli.quiet),
            TaskCommands::Update {
                id,
                title,
                description,
                status,
                priority,
                epic,
                assignee,
                due,
                tags,
                notes,
                user_info,
                agent_questions,
            } => commands::task::update(
                id,
                title.as_deref(),
                description.as_deref(),
                status.as_deref(),
                priority.as_deref(),
                epic,
                assignee,
                due.as_deref(),
                tags.as_deref(),
                notes.as_deref(),
                user_info.as_deref(),
                agent_questions.as_deref(),
                &cli.format,
                cli.quiet,
            ),
            TaskCommands::Delete { id, force } => commands::task::delete(id, force, cli.quiet),
            TaskCommands::Assign {
                task_id,
                assignee_id,
            } => commands::task::assign(task_id, assignee_id, cli.quiet),
            TaskCommands::Link(cmd) => match cmd {
                LinkCommands::GithubIssue { task, reference } => {
                    commands::task::link_github_issue(task, &reference, cli.quiet)
                }
                LinkCommands::GithubPr { task, reference } => {
                    commands::task::link_github_pr(task, &reference, cli.quiet)
                }
                LinkCommands::Url { task, url } => commands::task::link_url(task, &url, cli.quiet),
                LinkCommands::Blocks { task, blocked } => {
                    commands::task::link_blocks(task, blocked, cli.quiet)
                }
            },
            TaskCommands::Unlink { ref_id } => commands::task::unlink_ref(ref_id, cli.quiet),
            TaskCommands::Close { id, force } => commands::task::close(id, force, cli.quiet),
            TaskCommands::Reopen { id } => commands::task::reopen(id, cli.quiet),
            TaskCommands::Note { task_id, content } => {
                commands::task::add_note(task_id, &content, &cli.format, cli.quiet)
            }
            TaskCommands::Notes { task_id } => {
                commands::task::show_notes(task_id, &cli.format, cli.quiet)
            }
            TaskCommands::Clone { id, title } => {
                commands::task::clone_task(id, title.as_deref(), &cli.format, cli.quiet)
            }
            TaskCommands::BatchOp(cmd) => match cmd {
                BatchOpCommands::Close { ids, force } => {
                    commands::task::batch_close(&ids, force, cli.quiet)
                }
                BatchOpCommands::Tag { tag, ids } => {
                    commands::task::batch_tag(&tag, &ids, cli.quiet)
                }
                BatchOpCommands::Move { epic_id, ids } => {
                    commands::task::batch_move(epic_id, &ids, cli.quiet)
                }
                BatchOpCommands::DeleteOrphans { force } => {
                    commands::task::batch_delete_orphans(force, cli.quiet)
                }
            },
        },

        Commands::Assignee(cmd) => match cmd {
            AssigneeCommands::Create {
                name,
                email,
                github,
            } => commands::assignee::create(
                &name,
                email.as_deref(),
                github.as_deref(),
                &cli.format,
                cli.quiet,
            ),
            AssigneeCommands::List => commands::assignee::list(&cli.format, cli.quiet),
            AssigneeCommands::Show { id } => commands::assignee::show(id, &cli.format, cli.quiet),
            AssigneeCommands::Delete { id, force } => {
                commands::assignee::delete(id, force, cli.quiet)
            }
        },

        Commands::Deps(cmd) => match cmd {
            DepsCommands::Show { task_id } => commands::deps::show(task_id, &cli.format, cli.quiet),
            DepsCommands::Unlink {
                task_id,
                blocked_task_id,
            } => commands::deps::unlink(task_id, blocked_task_id, cli.quiet),
        },

        Commands::List(args) => commands::list::execute(
            args.epic,
            args.status.as_deref(),
            args.priority.as_deref(),
            args.assignee,
            args.blocked,
            args.overdue,
            args.tag.as_deref(),
            args.all,
            &cli.format,
            cli.quiet,
        ),

        Commands::Summary => commands::summary::execute(&cli.format, cli.quiet),

        Commands::Export(cmd) => match cmd {
            ExportCommands::Json { output } => commands::export::json(output.as_deref(), cli.quiet),
            ExportCommands::Csv { output } => commands::export::csv(output.as_deref(), cli.quiet),
        },

        Commands::Doctor { fix } => commands::doctor::execute(fix, cli.quiet),

        Commands::Linear(cmd) => match cmd {
            LinearCommands::Setup => commands::linear::setup(cli.quiet),
            LinearCommands::Sync {
                force_local,
                force_remote,
            } => commands::linear::sync_cmd(force_local, force_remote, cli.quiet),
            LinearCommands::Push => commands::linear::push_cmd(cli.quiet),
            LinearCommands::Pull => commands::linear::pull_cmd(cli.quiet),
            LinearCommands::Status => commands::linear::status_cmd(&cli.format, cli.quiet),
            LinearCommands::Unlink => commands::linear::unlink_cmd(cli.quiet),
        },

        Commands::Followup(cmd) => match cmd {
            FollowupCommands::Add { body, title } => {
                commands::followup::add(&body, title.as_deref(), &cli.format, cli.quiet)
            }
            FollowupCommands::List {
                status,
                all,
                open,
                closed,
            } => commands::followup::list(
                status.as_deref(),
                all,
                open,
                closed,
                &cli.format,
                cli.quiet,
            ),
            FollowupCommands::Show { id } => commands::followup::show(id, &cli.format, cli.quiet),
            FollowupCommands::Next => commands::followup::next(&cli.format, cli.quiet),
            FollowupCommands::Start { id } => commands::followup::set_status(
                id,
                models::FollowupStatus::InProgress,
                None,
                cli.quiet,
            ),
            FollowupCommands::Done { id, reason } => commands::followup::set_status(
                id,
                models::FollowupStatus::Done,
                reason.as_deref(),
                cli.quiet,
            ),
            FollowupCommands::Wontfix { id, reason } => commands::followup::set_status(
                id,
                models::FollowupStatus::Wontfix,
                reason.as_deref(),
                cli.quiet,
            ),
            FollowupCommands::Reopen { id } => {
                commands::followup::set_status(id, models::FollowupStatus::Open, None, cli.quiet)
            }
            FollowupCommands::Edit { id, body, title } => {
                let clear_title = title.as_deref() == Some("-");
                let title_arg = if clear_title { None } else { title.as_deref() };
                commands::followup::edit(id, body.as_deref(), title_arg, clear_title, cli.quiet)
            }
            FollowupCommands::Append { id, text } => {
                commands::followup::append(id, &text, cli.quiet)
            }
            FollowupCommands::Rm { id, force } => commands::followup::remove(id, force, cli.quiet),
            FollowupCommands::Promote { id, epic, priority } => {
                commands::followup::promote(id, epic, &priority, cli.quiet)
            }
            FollowupCommands::Count => commands::followup::count(&cli.format, cli.quiet),
            FollowupCommands::Snooze { turns } => commands::followup::snooze(turns, cli.quiet),
        },

        Commands::Hooks(cmd) => match cmd {
            HooksCommands::Install { global } => commands::hooks::install(scope(global)),
            HooksCommands::Uninstall { global } => commands::hooks::uninstall(scope(global)),
            HooksCommands::Status => commands::hooks::status(),
        },

        Commands::Update => commands::update::execute(),

        Commands::Version => commands::version::execute(&cli.format, cli.quiet),
    };

    if let Err(e) = result {
        handle_error(e);
    }
}
