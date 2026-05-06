# Semantic Search Feature Specification

## Overview
Extend Mycelium with semantic search capabilities using local embeddings, enabling both task management AND knowledge base use cases.

## Requirements

### 1. Embedding Engine
- **Primary**: Local embedding model (ONNX runtime)
- **Model**: Small model like `all-MiniLM-L6-v2` (384 dims) or similar
- **Fallback**: Configurable external API (OpenAI, etc)
- **Config**: `embedding.provider = "local" | "openai"` in mycelium config

### 2. Scope
Index ALL content:
- Tasks (title, description, notes, user_info, agent_questions)
- Epics (title, description, notes, user_info, agent_questions)
- Task Notes (content)
- Epic Notes (content)

### 3. Database Schema
```sql
CREATE TABLE embeddings (
    id INTEGER PRIMARY KEY,
    entity_type TEXT NOT NULL,  -- 'task', 'epic', 'task_note', 'epic_note'
    entity_id INTEGER NOT NULL,
    content_hash TEXT NOT NULL, -- SHA256 of searchable content for invalidation
    embedding BLOB NOT NULL,    -- f32 array (384 dims for local, 1536 for OpenAI)
    model_version TEXT,         -- e.g., "all-MiniLM-L6-v2"
    created_at TEXT
);
CREATE INDEX idx_embeddings_entity ON embeddings(entity_type, entity_id);
CREATE INDEX idx_embeddings_hash ON embeddings(content_hash);
```

### 4. New Fields in Models

Add to Task and Epic models:
```rust
pub is_knowledge: bool,      // Flag for knowledge-base items
pub key_questions: Option<String>, // Key questions this item answers
```

### 5. CLI Commands

```bash
# Indexing
myc embed <task-id>           # Index specific task
myc embed --epic <epic-id>    # Index specific epic
myc embed --all               # Index all unindexed items
myc embed --force <id>        # Re-index even if hash matches

# Semantic Search
myc search "query" --semantic [--top N]
myc search "query" --hybrid   # FTS + semantic combined

# Knowledge-specific
myc knowledge list            # List items marked as knowledge
myc knowledge search "query"  # Search only knowledge items
myc task create --knowledge   # Create as knowledge item
```

### 6. Searchable Content Generation

Function to concatenate all searchable text:
```rust
fn get_searchable_content(entity) -> String {
    format!("{} {} {} {} {}", 
        title,
        description.unwrap_or(""),
        notes.unwrap_or(""),
        user_info.unwrap_or(""),
        agent_questions.unwrap_or("")
    )
}
```

### 7. Similarity Search

Use cosine similarity:
```rust
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}
```

### 8. Dependencies to Add

```toml
[dependencies]
# ONNX runtime for local embeddings
ort = { version = "2.0", features = ["load-dynamic"] }
# Tokenization
tokenizers = "0.15"
# Async runtime for embedding generation
tokio = { version = "1.0", features = ["rt-multi-thread"] }
```

### 9. Auto-embedding Configuration

Add to config:
```toml
[semantic_search]
enabled = true
auto_embed = true  # Auto-index on create/update
model = "all-MiniLM-L6-v2"  # Local model path or name
embedding_dims = 384
provider = "local"  # or "openai"
```

### 10. Implementation Order

1. Add migrations for embeddings table
2. Add `is_knowledge` and `key_questions` to task/epic models
3. Create embedding module (local ONNX inference)
4. Add embed commands
5. Add semantic search command
6. Add hybrid search (combine FTS + semantic)
7. Add knowledge-specific commands
8. Add auto-embed hooks

## Notes

- Keep embeddings in separate table to not bloat main tables
- Use content_hash to avoid re-indexing unchanged items
- Local model should be downloadable on first use (~80MB)
- Consider batching for --all indexing
