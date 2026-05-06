use std::path::PathBuf;

use crate::error::{MyceliumError, Result};

const MODEL_NAME: &str = "Xenova/all-MiniLM-L6-v2";
pub const MODEL_VERSION: &str = "all-MiniLM-L6-v2";
pub const EMBEDDING_DIMS: usize = 384;

/// Get the cache directory for model files.
fn model_cache_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "mycelium")
        .ok_or_else(|| MyceliumError::Embedding("Cannot determine home directory".into()))?;
    let cache = dirs.cache_dir().join("models");
    std::fs::create_dir_all(&cache)?;
    Ok(cache)
}

/// Download the ONNX model and tokenizer if not cached, return paths.
fn ensure_model_files() -> Result<(PathBuf, PathBuf)> {
    let cache = model_cache_dir()?;
    let model_path = cache.join("model.onnx");
    let tokenizer_path = cache.join("tokenizer.json");

    if model_path.exists() && tokenizer_path.exists() {
        return Ok((model_path, tokenizer_path));
    }

    eprintln!("Downloading embedding model ({})...", MODEL_NAME);

    let api = hf_hub::api::sync::Api::new()
        .map_err(|e| MyceliumError::Embedding(format!("HF Hub API error: {}", e)))?;
    let repo = api.model(MODEL_NAME.to_string());

    let onnx = repo.get("onnx/model.onnx")
        .map_err(|e| MyceliumError::Embedding(format!("Failed to download model.onnx: {}", e)))?;
    let tok = repo.get("tokenizer.json")
        .map_err(|e| MyceliumError::Embedding(format!("Failed to download tokenizer.json: {}", e)))?;

    // Copy to our cache location
    if onnx != model_path {
        std::fs::copy(&onnx, &model_path)?;
    }
    if tok != tokenizer_path {
        std::fs::copy(&tok, &tokenizer_path)?;
    }

    eprintln!("Model downloaded successfully.");
    Ok((model_path, tokenizer_path))
}

/// Compute embeddings for a list of texts using the Python sentence-transformers bridge.
pub fn embed_texts(texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    let hermes_home = std::env::var("HERMES_HOME")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{}/.hermes", home)
        });
    let script_path = std::path::Path::new(&hermes_home).join("mycelium_embed.py");

    // Prefer python3.14 (where sentence-transformers is installed), fallback to python3
    let python_cmd = if std::process::Command::new("python3.14").arg("--version").output().is_ok() {
        "python3.14"
    } else {
        "python3"
    };

    let input_json = serde_json::to_string(texts)
        .map_err(|e| MyceliumError::Embedding(format!("JSON serialize error: {}", e)))?;

    let mut child = std::process::Command::new(python_cmd)
        .arg(&script_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| MyceliumError::Embedding(format!("Failed to spawn Python bridge: {}", e)))?;

    use std::io::Write;
    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            MyceliumError::Embedding("Failed to open stdin for Python bridge".into())
        })?;
        stdin.write_all(input_json.as_bytes())
            .map_err(|e| MyceliumError::Embedding(format!("Failed to write to Python bridge: {}", e)))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| MyceliumError::Embedding(format!("Python bridge failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(MyceliumError::Embedding(format!(
            "Python bridge exited with error: {}",
            stderr.trim()
        )));
    }

    let embeddings: Vec<Vec<f32>> = serde_json::from_slice(&output.stdout)
        .map_err(|e| MyceliumError::Embedding(format!(
            "Failed to parse Python bridge output: {} (output: {})",
            e,
            String::from_utf8_lossy(&output.stdout).trim()
        )))?;

    if embeddings.len() != texts.len() {
        return Err(MyceliumError::Embedding(format!(
            "Embedding count mismatch: expected {}, got {}",
            texts.len(),
            embeddings.len()
        )));
    }

    Ok(embeddings)
}

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

/// Generate a SHA256 hash of content for change detection.
pub fn content_hash(content: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Build searchable content string from a task.
pub fn task_searchable_content(task: &crate::models::Task) -> String {
    let parts: Vec<&str> = [
        Some(task.title.as_str()),
        task.description.as_deref(),
        task.notes.as_deref(),
        task.user_info.as_deref(),
        task.agent_questions.as_deref(),
        task.key_questions.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    parts.join(" ")
}

/// Build searchable content string from an epic.
pub fn epic_searchable_content(epic: &crate::models::Epic) -> String {
    let parts: Vec<&str> = [
        Some(epic.title.as_str()),
        epic.description.as_deref(),
        epic.notes.as_deref(),
        epic.user_info.as_deref(),
        epic.agent_questions.as_deref(),
        epic.key_questions.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    parts.join(" ")
}
