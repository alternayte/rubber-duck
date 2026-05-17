use std::path::PathBuf;
use std::sync::Mutex;

use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};

use crate::error::{AppError, AppResult};

pub struct Embedder {
    model: Mutex<Option<TextEmbedding>>,
    model_dir: PathBuf,
}

impl Embedder {
    pub fn new(model_dir: PathBuf) -> Self {
        Self {
            model: Mutex::new(None),
            model_dir,
        }
    }

    pub fn embed(&self, texts: &[String]) -> AppResult<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }
        let mut guard = self
            .model
            .lock()
            .map_err(|e| AppError::Other(e.to_string()))?;

        // Lazy initialise on first use.
        if guard.is_none() {
            let m = TextEmbedding::try_new(
                TextInitOptions::new(EmbeddingModel::AllMiniLML6V2)
                    .with_cache_dir(self.model_dir.clone())
                    .with_show_download_progress(false),
            )
            .map_err(|e| AppError::Other(format!("Failed to load embedding model: {e}")))?;
            *guard = Some(m);
        }

        // SAFETY: we just ensured the Option is Some.
        let model = guard.as_mut().expect("model must be Some after init");

        model
            .embed(texts, None)
            .map_err(|e| AppError::Other(format!("Embedding failed: {e}")))
    }
}
