//! Embedding generation for semantic search.
//!
//! Provides embedding generation using fastembed (when available) or a
//! hash-based fallback for deterministic pseudo-embeddings.
//!
//! # Feature Flags
//!
//! - `fastembed-embeddings`: Enables `FastEmbed` with BGE-M3 (1024 dimensions, 8192 token max)
//! - Without the feature: Uses hash-based fallback (deterministic but not semantic)

mod fallback;

#[cfg(feature = "fastembed-embeddings")]
mod fastembed_impl;

pub use fallback::FallbackEmbedder;

#[cfg(feature = "fastembed-embeddings")]
pub use fastembed_impl::FastEmbedEmbedder;

use crate::Result;

/// Default embedding dimensions for the BGE-M3 model.
///
/// This is the authoritative source for embedding dimensions across the codebase.
/// All vector backends should use this constant for consistency.
pub const DEFAULT_DIMENSIONS: usize = 1024;

/// Trait for embedding generators.
///
/// Implementations must be thread-safe (`Send + Sync`) to support parallel
/// embedding generation during chunk loading.
///
/// # Examples
///
/// ```
/// use rlm_rs::embedding::{Embedder, FallbackEmbedder, DEFAULT_DIMENSIONS};
///
/// let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
/// let embedding = embedder.embed("Hello, world!").unwrap();
/// assert_eq!(embedding.len(), DEFAULT_DIMENSIONS);
/// ```
pub trait Embedder: Send + Sync {
    /// Returns the embedding dimensions.
    fn dimensions(&self) -> usize;

    /// Generates an embedding for the given text.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding generation fails.
    fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generates embeddings for multiple texts.
    ///
    /// The default implementation calls `embed` for each text sequentially.
    /// Implementations may override this for batch optimization.
    ///
    /// # Errors
    ///
    /// Returns an error if embedding generation fails for any text.
    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }
}

/// Creates the default embedder based on available features.
///
/// - With `fastembed-embeddings`: Returns `FastEmbedEmbedder`
/// - Without: Returns `FallbackEmbedder`
///
/// # Errors
///
/// Returns an error if embedder initialization fails.
#[cfg(feature = "fastembed-embeddings")]
pub fn create_embedder() -> Result<Box<dyn Embedder>> {
    Ok(Box::new(FastEmbedEmbedder::new()?))
}

/// Creates the default embedder based on available features.
///
/// - With `fastembed-embeddings`: Returns `FastEmbedEmbedder`
/// - Without: Returns `FallbackEmbedder`
///
/// # Errors
///
/// Returns an error if embedder initialization fails (never fails for fallback).
#[cfg(not(feature = "fastembed-embeddings"))]
pub fn create_embedder() -> Result<Box<dyn Embedder>> {
    Ok(Box::new(FallbackEmbedder::new(DEFAULT_DIMENSIONS)))
}

/// Computes cosine similarity between two embedding vectors.
///
/// Returns a value between -1.0 (opposite) and 1.0 (identical).
/// For normalized vectors (L2 norm = 1), this is equivalent to the dot product.
///
/// # Panics
///
/// Does not panic but returns 0.0 if vectors have different lengths or zero magnitude.
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_vector() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_create_embedder() {
        let embedder = create_embedder().unwrap();
        assert_eq!(embedder.dimensions(), DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_embed_batch_default_impl() {
        // Test the default embed_batch implementation (lines 62-63)
        let embedder = create_embedder().unwrap();
        let texts = vec!["hello", "world", "test"];
        let embeddings = embedder.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for embedding in &embeddings {
            assert_eq!(embedding.len(), embedder.dimensions());
        }
    }

    #[test]
    fn test_embed_batch_empty() {
        // Test embed_batch with empty slice
        let embedder = create_embedder().unwrap();
        let texts: Vec<&str> = vec![];
        let embeddings = embedder.embed_batch(&texts).unwrap();
        assert!(embeddings.is_empty());
    }
}
