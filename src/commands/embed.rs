use colored::Colorize;
use crate::db::Database;
use crate::embedding::{self, MODEL_VERSION};
use crate::error::Result;
use super::{ensure_initialized, SUCCESS_PREFIX, INFO_PREFIX, WARNING_PREFIX};

pub fn embed_task(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let task = db.get_task(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "task".into(),
            id: id.to_string(),
        })?;

    let content = embedding::task_searchable_content(&task);
    let hash = embedding::content_hash(&content);

    if !force {
        if let Some(existing_hash) = db.get_embedding_hash("task", id)? {
            if existing_hash == hash {
                if !quiet {
                    println!("{} Task {} already indexed (content unchanged)", INFO_PREFIX, id);
                }
                return Ok(());
            }
        }
    }

    let embeddings = embedding::embed_texts(&[content.as_str()])?;
    db.upsert_embedding("task", id, &hash, &embeddings[0], MODEL_VERSION)?;

    if !quiet {
        println!("{} Indexed task {} ({})", SUCCESS_PREFIX, id, task.title);
    }
    Ok(())
}

pub fn embed_epic(id: i64, force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let epic = db.get_epic(id)?
        .ok_or_else(|| crate::error::MyceliumError::NotFound {
            entity: "epic".into(),
            id: id.to_string(),
        })?;

    let content = embedding::epic_searchable_content(&epic);
    let hash = embedding::content_hash(&content);

    if !force {
        if let Some(existing_hash) = db.get_embedding_hash("epic", id)? {
            if existing_hash == hash {
                if !quiet {
                    println!("{} Epic {} already indexed (content unchanged)", INFO_PREFIX, id);
                }
                return Ok(());
            }
        }
    }

    let embeddings = embedding::embed_texts(&[content.as_str()])?;
    db.upsert_embedding("epic", id, &hash, &embeddings[0], MODEL_VERSION)?;

    if !quiet {
        println!("{} Indexed epic {} ({})", SUCCESS_PREFIX, id, epic.title);
    }
    Ok(())
}

pub fn embed_all(force: bool, quiet: bool) -> Result<()> {
    let mut db = ensure_initialized()?;

    let tasks = db.list_all_tasks()?;
    let epics = db.list_epics()?;

    let mut indexed = 0;
    let mut skipped = 0;

    // Collect texts to embed in batches
    let mut to_embed: Vec<(&str, i64, String, String)> = Vec::new(); // (entity_type, entity_id, content, hash)

    for task in &tasks {
        let content = embedding::task_searchable_content(task);
        let hash = embedding::content_hash(&content);

        if !force {
            if let Some(existing_hash) = db.get_embedding_hash("task", task.id)? {
                if existing_hash == hash {
                    skipped += 1;
                    continue;
                }
            }
        }
        to_embed.push(("task", task.id, content, hash));
    }

    for epic in &epics {
        let content = embedding::epic_searchable_content(epic);
        let hash = embedding::content_hash(&content);

        if !force {
            if let Some(existing_hash) = db.get_embedding_hash("epic", epic.id)? {
                if existing_hash == hash {
                    skipped += 1;
                    continue;
                }
            }
        }
        to_embed.push(("epic", epic.id, content, hash));
    }

    if to_embed.is_empty() {
        if !quiet {
            println!("{} Everything is already indexed ({} items)", INFO_PREFIX, skipped);
        }
        return Ok(());
    }

    if !quiet {
        println!("{} Embedding {} items...", INFO_PREFIX, to_embed.len());
    }

    // Batch embed (process in chunks to avoid OOM on large collections)
    let batch_size = 32;
    for chunk in to_embed.chunks(batch_size) {
        let texts: Vec<&str> = chunk.iter().map(|(_, _, content, _)| content.as_str()).collect();
        let embeddings = embedding::embed_texts(&texts)?;

        for (i, (entity_type, entity_id, _, hash)) in chunk.iter().enumerate() {
            db.upsert_embedding(entity_type, *entity_id, hash, &embeddings[i], MODEL_VERSION)?;
            indexed += 1;
        }
    }

    if !quiet {
        println!("{} Indexed {} items, skipped {} unchanged", SUCCESS_PREFIX, indexed, skipped);
    }
    Ok(())
}

pub fn status(quiet: bool) -> Result<()> {
    let db = ensure_initialized()?;

    let count = db.count_embeddings()?;
    let total_tasks = db.list_all_tasks()?.len();
    let total_epics = db.list_epics()?.len();
    let total = total_tasks + total_epics;

    if !quiet {
        println!("{} Embedding index status:", INFO_PREFIX);
        println!("  Indexed: {}/{} items", count, total);
        println!("  Tasks:   {} total", total_tasks);
        println!("  Epics:   {} total", total_epics);
        if count < total as i64 {
            println!("  Run {} to index remaining items", "myc embed all".bold());
        }
    }
    Ok(())
}
