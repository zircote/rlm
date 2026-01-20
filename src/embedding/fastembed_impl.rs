//! `FastEmbed`-based semantic embedder.
//!
//! Provides real semantic embeddings using the BGE-M3 model via fastembed-rs.
//! Only available when the `fastembed-embeddings` feature is enabled.

use crate::Result;
use crate::embedding::{DEFAULT_DIMENSIONS, Embedder};
use crate::error::StorageError;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::OnceLock;

/// Thread-safe singleton for the embedding model.
/// Uses `OnceLock` for lazy initialization on first use.
static EMBEDDING_MODEL: OnceLock<std::sync::Mutex<fastembed::TextEmbedding>> = OnceLock::new();

/// `FastEmbed` embedder using BGE-M3.
///
/// Uses the fastembed-rs library for real semantic embeddings.
/// The model is lazily loaded on first embed call to preserve cold start time.
///
/// BGE-M3 provides:
/// - 1024 dimensions (vs 384 for `MiniLM`)
/// - 8192 token context (vs ~512 for `MiniLM`)
/// - Better multilingual support
///
/// # Examples
///
/// ```ignore
/// use rlm_rs::embedding::FastEmbedEmbedder;
///
/// let embedder = FastEmbedEmbedder::new()?;
/// let embedding = embedder.embed("Hello, world!")?;
/// assert_eq!(embedding.len(), 1024);
/// ```
pub struct FastEmbedEmbedder {
    /// Model name for debugging.
    model_name: &'static str,
}

impl FastEmbedEmbedder {
    /// Creates a new `FastEmbed` embedder.
    ///
    /// Note: Model is lazily loaded on first `embed()` call.
    ///
    /// # Errors
    ///
    /// Returns an error if model initialization fails.
    #[allow(clippy::missing_const_for_fn)]
    pub fn new() -> Result<Self> {
        Ok(Self {
            model_name: "BGE-M3",
        })
    }

    /// Gets or initializes the embedding model (thread-safe).
    ///
    /// The model is loaded lazily on first use to preserve cold start time.
    /// Subsequent calls return the cached instance.
    fn get_model() -> Result<&'static std::sync::Mutex<fastembed::TextEmbedding>> {
        // Check if already initialized
        if let Some(model) = EMBEDDING_MODEL.get() {
            return Ok(model);
        }

        // Initialize the model
        let options = fastembed::InitOptions::new(fastembed::EmbeddingModel::BGEM3)
            .with_show_download_progress(false);

        let model = fastembed::TextEmbedding::try_new(options).map_err(|e| {
            StorageError::Transaction(format!("Failed to load embedding model: {e}"))
        })?;

        // Store the model, ignoring if another thread beat us to it
        let _ = EMBEDDING_MODEL.set(std::sync::Mutex::new(model));

        // Return the (possibly other thread's) model
        EMBEDDING_MODEL.get().ok_or_else(|| {
            StorageError::Transaction("Model initialization race condition".to_string()).into()
        })
    }

    /// Returns the model name.
    #[must_use]
    pub const fn model_name(&self) -> &'static str {
        self.model_name
    }
}

impl Embedder for FastEmbedEmbedder {
    fn dimensions(&self) -> usize {
        DEFAULT_DIMENSIONS
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if text.is_empty() {
            return Err(crate::Error::Chunking(
                crate::error::ChunkingError::InvalidConfig {
                    reason: "Cannot embed empty text".to_string(),
                },
            ));
        }

        let model = Self::get_model()?;
        let mut model = model.lock().map_err(|e| {
            StorageError::Transaction(format!("Failed to lock embedding model: {e}"))
        })?;

        let texts = [text];

        // Wrap ONNX runtime call in catch_unwind for graceful degradation.
        // ONNX runtime can panic on malformed inputs or internal errors.
        let result = catch_unwind(AssertUnwindSafe(|| model.embed(texts, None)));

        let embeddings = result
            .map_err(|panic_info| {
                let panic_msg = panic_info
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| panic_info.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_string());
                StorageError::Transaction(format!("ONNX runtime panic: {panic_msg}"))
            })?
            .map_err(|e| StorageError::Transaction(format!("Embedding failed: {e}")))?;

        embeddings.into_iter().next().ok_or_else(|| {
            StorageError::Transaction("No embedding returned from model".to_string()).into()
        })
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        if texts.iter().any(|t| t.is_empty()) {
            return Err(crate::Error::Chunking(
                crate::error::ChunkingError::InvalidConfig {
                    reason: "Cannot embed empty text".to_string(),
                },
            ));
        }

        let model = Self::get_model()?;
        let mut model = model.lock().map_err(|e| {
            StorageError::Transaction(format!("Failed to lock embedding model: {e}"))
        })?;

        // Wrap ONNX runtime call in catch_unwind for graceful degradation.
        let result = catch_unwind(AssertUnwindSafe(|| model.embed(texts, None)));

        result
            .map_err(|panic_info| {
                let panic_msg = panic_info
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| panic_info.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_string());
                crate::Error::Storage(StorageError::Transaction(format!(
                    "ONNX runtime panic: {panic_msg}"
                )))
            })?
            .map_err(|e| {
                crate::Error::Storage(StorageError::Transaction(format!(
                    "Batch embedding failed: {e}"
                )))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_creation() {
        let embedder = FastEmbedEmbedder::new();
        assert!(embedder.is_ok());
        assert_eq!(embedder.unwrap().dimensions(), DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_model_name() {
        let embedder = FastEmbedEmbedder::new().unwrap();
        assert_eq!(embedder.model_name(), "BGE-M3");
    }

    // Integration tests that require model download are marked #[ignore]
    // Run with: cargo test --features fastembed-embeddings -- --ignored

    #[test]
    #[ignore = "requires fastembed model download"]
    fn test_embed_success() {
        let embedder = FastEmbedEmbedder::new().unwrap();
        let result = embedder.embed("Hello, world!");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), DEFAULT_DIMENSIONS);
    }

    #[test]
    #[ignore = "requires fastembed model download"]
    fn test_embed_batch_success() {
        let embedder = FastEmbedEmbedder::new().unwrap();
        let texts = vec!["Hello", "World"];
        let result = embedder.embed_batch(&texts);
        assert!(result.is_ok());
        let embeddings = result.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embed_empty_fails() {
        let embedder = FastEmbedEmbedder::new().unwrap();
        let result = embedder.embed("");
        assert!(result.is_err());
    }

    #[test]
    fn test_embed_batch_empty_list() {
        let embedder = FastEmbedEmbedder::new().unwrap();
        let result = embedder.embed_batch(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_embed_batch_with_empty_fails() {
        let embedder = FastEmbedEmbedder::new().unwrap();
        let texts = vec!["Valid", "", "Also valid"];
        let result = embedder.embed_batch(&texts);
        assert!(result.is_err());
    }
}
