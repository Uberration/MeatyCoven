//! Document ingestion — chunking, hashing, dedup, embedding, indexing

use anyhow::Result;
use std::path::Path;
use sha2::{Digest, Sha256};
use crate::{MemoryDoc, db::MetaDb, embed::Embedder, index::VecIndex};

/// Chunk size in characters (~400 chars ≈ ~100 tokens — good for 768-dim nomic)
const CHUNK_SIZE: usize = 400;
/// Overlap between chunks
const CHUNK_OVERLAP: usize = 80;

/// Ingest a single file into the memory layer
/// Returns number of new chunks added (0 if all chunks already present)
pub fn ingest_file(
    path: &Path,
    familiar: &str,
    db: &MetaDb,
    index: &mut VecIndex,
    embedder: &mut Embedder,
) -> Result<usize> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("reading {}: {}", path.display(), e))?;

    let chunks = chunk_text(&text);
    let path_str = path.to_string_lossy().to_string();

    let mut new_chunks: Vec<(usize, String)> = Vec::new(); // (offset, chunk)

    for (offset, chunk) in &chunks {
        let hash = hex_hash(chunk);
        if !db.hash_exists(&hash)? {
            new_chunks.push((*offset, chunk.clone()));
        }
    }

    if new_chunks.is_empty() {
        return Ok(0);
    }

    // Embed all new chunks in one batch
    let texts: Vec<&str> = new_chunks.iter().map(|(_, c)| c.as_str()).collect();
    let embeddings = embedder.embed_batch(&texts)?;

    let mut ids = Vec::new();
    let mut vecs = Vec::new();

    for ((offset, chunk), embedding) in new_chunks.iter().zip(embeddings.iter()) {
        let hash = hex_hash(chunk);
        let doc = MemoryDoc {
            id: 0, // filled after insert
            path: path_str.clone(),
            familiar: familiar.to_string(),
            chunk: chunk.clone(),
            chunk_offset: *offset,
            content_hash: hash,
            ingested_at: chrono::Utc::now().timestamp(),
        };
        let id = db.insert(&doc)?;
        ids.push(id);
        vecs.push(embedding.clone());
    }

    index.add(&ids, &vecs)?;
    Ok(ids.len())
}

/// Ingest all markdown files under a directory
pub fn ingest_dir(
    dir: &Path,
    familiar: &str,
    db: &MetaDb,
    index: &mut VecIndex,
    embedder: &mut Embedder,
) -> Result<(usize, usize)> {
    let mut files = 0usize;
    let mut chunks = 0usize;

    for entry in walkdir::WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().map_or(false, |ext| {
                    matches!(ext.to_str(), Some("md" | "txt" | "toml" | "rs"))
                })
        })
    {
        match ingest_file(entry.path(), familiar, db, index, embedder) {
            Ok(n) => {
                if n > 0 {
                    files += 1;
                    chunks += n;
                }
            }
            Err(e) => eprintln!("warn: skipping {}: {e}", entry.path().display()),
        }
    }
    Ok((files, chunks))
}

/// Sliding window chunker — returns (byte_offset, chunk_text) pairs
fn chunk_text(text: &str) -> Vec<(usize, String)> {
    let mut chunks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + CHUNK_SIZE).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        let trimmed = chunk.trim().to_string();
        if !trimmed.is_empty() {
            chunks.push((start, trimmed));
        }
        if end == chars.len() { break; }
        start += CHUNK_SIZE - CHUNK_OVERLAP;
    }
    chunks
}

fn hex_hash(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}
