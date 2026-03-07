use colored::Colorize;
use comfy_table::{Table, ContentArrangement};
use crate::commands::{ensure_initialized, INFO_PREFIX};
use crate::cli::OutputFormat;
use crate::error::Result;

pub fn execute(format: &OutputFormat, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;
    let summary = db.get_summary()?;
    
    if quiet {
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&summary)?),
        OutputFormat::Table => {
            println!("{} Project Summary", INFO_PREFIX.blue());
            println!();
            
            let mut table = Table::new();
            table.set_content_arrangement(ContentArrangement::Dynamic);
            table.set_header(vec!["Metric", "Count"]);
            
            table.add_row(vec!["Total Epics", &summary.total_epics.to_string()]);
            table.add_row(vec!["Open Epics", &summary.open_epics.to_string()]);
            table.add_row(vec!["Closed Epics", &summary.closed_epics.to_string()]);
            table.add_row(vec!["", ""]);
            table.add_row(vec!["Total Tasks", &summary.total_tasks.to_string()]);
            table.add_row(vec!["Open Tasks", &summary.open_tasks.to_string()]);
            table.add_row(vec!["Closed Tasks", &summary.closed_tasks.to_string()]);
            table.add_row(vec!["", ""]);
            table.add_row(vec!["Overdue Tasks", &summary.overdue_tasks.to_string().red().to_string()]);
            table.add_row(vec!["Blocked Tasks", &summary.blocked_tasks.to_string().yellow().to_string()]);
            
            println!("{}", table);
            
            if summary.total_tasks > 0 {
                let completion = (summary.closed_tasks * 100 / summary.total_tasks) as f64;
                println!("\nOverall completion: {:.1}%", completion);
            }
        }
    }
    Ok(())
}
