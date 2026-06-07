//! Local embedding via fastembed — nomic-embed-text-v1.5 (768-dim, ONNX, air-gapped)

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

pub const DIM: usize = 768;

pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    /// Initialise — downloads model on first run (~270 MB), cached after.
    pub fn new() -> Result<Self> {
        let opts = TextInitOptions::new(EmbeddingModel::NomicEmbedTextV15)
            .with_show_download_progress(true);
        let model = TextEmbedding::try_new(opts)
            .context("initialising fastembed (nomic-embed-text-v1.5)")?;
        Ok(Self { model })
    }

    /// Embed a batch of texts — returns Vec<Vec<f32>>, each of length DIM
    pub fn embed_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let embeddings = self.model
            .embed(texts.to_vec(), None)
            .context("embedding batch")?;
        Ok(embeddings)
    }

    /// Embed a single query string
    pub fn embed_query(&mut self, query: &str) -> Result<Vec<f32>> {
        let mut results = self.embed_batch(&[query])?;
        results.pop().context("empty embedding result")
    }
}
