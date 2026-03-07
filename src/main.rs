use clap::Parser;

mod cli;
mod commands;
mod db;
mod error;
mod models;

use cli::{Cli, Commands, EpicCommands, TaskCommands, AssigneeCommands, DepsCommands, ExportCommands, LinkCommands};
use error::handle_error;

pub use commands::{ERROR_PREFIX, SUCCESS_PREFIX, INFO_PREFIX, WARNING_PREFIX};

fn main() {
    let cli = Cli::parse();
    
    let result = match cli.command {
        Commands::Init => commands::init::execute(),
        
        Commands::Epic(cmd) => match cmd {
            EpicCommands::Create { title, description } => {
                commands::epic::create(&title, description.as_deref(), &cli.format, cli.quiet)
            }
            EpicCommands::List => {
                commands::epic::list(&cli.format, cli.quiet)
            }
            EpicCommands::Show { id } => {
                commands::epic::show(id, &cli.format, cli.quiet)
            }
            EpicCommands::Update { id, title, description, status } => {
                commands::epic::update(id, title.as_deref(), description.as_deref(), status.as_deref(), &cli.format, cli.quiet)
            }
            EpicCommands::Delete { id, force } => {
                commands::epic::delete(id, force, cli.quiet)
            }
        },
        
        Commands::Task(cmd) => match cmd {
            TaskCommands::Create { title, description, epic, priority, assignee, due, tags, template } => {
                commands::task::create(&title, description.as_deref(), epic, &priority, assignee, due.as_deref(), tags.as_deref(), template.as_deref(), &cli.format, cli.quiet)
            }
            TaskCommands::List { epic, status, priority, assignee, blocked, overdue, tag } => {
                commands::task::list(epic, status.as_deref(), priority.as_deref(), assignee, blocked, overdue, tag.as_deref(), &cli.format, cli.quiet)
            }
            TaskCommands::Batch { file } => {
                commands::task::batch(&file, &cli.format, cli.quiet)
            }
            TaskCommands::Show { id } => {
                commands::task::show(id, &cli.format, cli.quiet)
            }
            TaskCommands::Update { id, title, description, status, priority, epic, assignee, due, tags } => {
                commands::task::update(
                    id, title.as_deref(), description.as_deref(), status.as_deref(), 
                    priority.as_deref(), epic, assignee, due.as_deref(), tags.as_deref(), &cli.format, cli.quiet
                )
            }
            TaskCommands::Delete { id, force } => {
                commands::task::delete(id, force, cli.quiet)
            }
            TaskCommands::Assign { task_id, assignee_id } => {
                commands::task::assign(task_id, assignee_id, cli.quiet)
            }
            TaskCommands::Link(cmd) => match cmd {
                LinkCommands::GithubIssue { task, reference } => {
                    commands::task::link_github_issue(task, &reference, cli.quiet)
                }
                LinkCommands::GithubPr { task, reference } => {
                    commands::task::link_github_pr(task, &reference, cli.quiet)
                }
                LinkCommands::Url { task, url } => {
                    commands::task::link_url(task, &url, cli.quiet)
                }
                LinkCommands::Blocks { task, blocked } => {
                    commands::task::link_blocks(task, blocked, cli.quiet)
                }
            }
            TaskCommands::Unlink { ref_id } => {
                commands::task::unlink_ref(ref_id, cli.quiet)
            }
            TaskCommands::Close { id, force } => {
                commands::task::close(id, force, cli.quiet)
            }
            TaskCommands::Reopen { id } => {
                commands::task::reopen(id, cli.quiet)
            }
        },
        
        Commands::Assignee(cmd) => match cmd {
            AssigneeCommands::Create { name, email, github } => {
                commands::assignee::create(&name, email.as_deref(), github.as_deref(), &cli.format, cli.quiet)
            }
            AssigneeCommands::List => {
                commands::assignee::list(&cli.format, cli.quiet)
            }
            AssigneeCommands::Show { id } => {
                commands::assignee::show(id, &cli.format, cli.quiet)
            }
            AssigneeCommands::Delete { id, force } => {
                commands::assignee::delete(id, force, cli.quiet)
            }
        },
        
        Commands::Deps(cmd) => match cmd {
            DepsCommands::Show { task_id } => {
                commands::deps::show(task_id, &cli.format, cli.quiet)
            }
            DepsCommands::Unlink { task_id, blocked_task_id } => {
                commands::deps::unlink(task_id, blocked_task_id, cli.quiet)
            }
        },
        
        Commands::List(args) => {
            commands::list::execute(args.epic, args.status.as_deref(), args.priority.as_deref(), args.assignee, args.blocked, args.overdue, args.tag.as_deref(), &cli.format, cli.quiet)
        }
        
        Commands::Summary => {
            commands::summary::execute(&cli.format, cli.quiet)
        }
        
        Commands::Export(cmd) => match cmd {
            ExportCommands::Json { output } => {
                commands::export::json(output.as_deref(), cli.quiet)
            }
            ExportCommands::Csv { output } => {
                commands::export::csv(output.as_deref(), cli.quiet)
            }
        }
    };
    
    if let Err(e) = result {
        handle_error(e);
    }
}
