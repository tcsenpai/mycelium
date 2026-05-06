use colored::Colorize;
use crate::embedding;
use crate::error::Result;
use super::{ensure_initialized, INFO_PREFIX, WARNING_PREFIX};

pub fn semantic_search(query: &str, top_n: usize, knowledge_only: bool, quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;

    let count = db.count_embeddings()?;
    if count == 0 {
        if !quiet {
            println!("{} No embeddings found. Run {} first.", WARNING_PREFIX, "myc embed all".bold());
        }
        return Ok(());
    }

    // Embed the query
    let query_embeddings = embedding::embed_texts(&[query])?;
    let query_vec = &query_embeddings[0];

    // Get all embeddings and compute similarity
    let all_embeddings = db.get_all_embeddings()?;

    let mut scored: Vec<(f32, &str, i64)> = all_embeddings
        .iter()
        .map(|rec| {
            let sim = embedding::cosine_similarity(query_vec, &rec.embedding);
            (sim, rec.entity_type.as_str(), rec.entity_id)
        })
        .collect();

    // Sort by similarity descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Collect results
    let mut displayed = 0;

    if !quiet {
        println!("{} Semantic search results for: \"{}\"", INFO_PREFIX, query.bold());
        println!();
    }

    for (score, entity_type, entity_id) in &scored {
        if displayed >= top_n {
            break;
        }

        match *entity_type {
            "task" => {
                if let Some(task) = db.get_task(*entity_id)? {
                    if knowledge_only && !task.is_knowledge {
                        continue;
                    }
                    let knowledge_tag = if task.is_knowledge { " [knowledge]".dimmed().to_string() } else { String::new() };
                    let score_str = format!("{:.3}", score);
                    println!(
                        "  {} {} #{} {}{}",
                        score_str.dimmed(),
                        "task".cyan(),
                        entity_id,
                        task.title,
                        knowledge_tag,
                    );
                    if let Some(ref desc) = task.description {
                        let preview = if desc.len() > 100 { &desc[..100] } else { desc.as_str() };
                        println!("       {}", preview.dimmed());
                    }
                    displayed += 1;
                }
            }
            "epic" => {
                if let Some(epic) = db.get_epic(*entity_id)? {
                    if knowledge_only && !epic.is_knowledge {
                        continue;
                    }
                    let knowledge_tag = if epic.is_knowledge { " [knowledge]".dimmed().to_string() } else { String::new() };
                    let score_str = format!("{:.3}", score);
                    println!(
                        "  {} {} #{} {}{}",
                        score_str.dimmed(),
                        "epic".magenta(),
                        entity_id,
                        epic.title,
                        knowledge_tag,
                    );
                    if let Some(ref desc) = epic.description {
                        let preview = if desc.len() > 100 { &desc[..100] } else { desc.as_str() };
                        println!("       {}", preview.dimmed());
                    }
                    displayed += 1;
                }
            }
            _ => {}
        }
    }

    if displayed == 0 && !quiet {
        println!("  No results found.");
    } else if !quiet {
        println!();
        println!("  {} results shown (of {} indexed items)", displayed, count);
    }

    Ok(())
}
