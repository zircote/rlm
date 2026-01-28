//! Hash-based fallback embedder.
//!
//! Provides deterministic pseudo-embeddings when `FastEmbed` is not available.
//! Uses content hashing to generate reproducible embeddings that cluster
//! similar text together (based on word overlap, not semantics).

use crate::Result;
use crate::embedding::Embedder;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hash-based fallback embedder.
///
/// Generates deterministic pseudo-embeddings using a combination of:
/// - Word-level hashing for vocabulary capture
/// - Character n-gram hashing for fuzzy matching
/// - Normalization to unit length for cosine similarity
///
/// This is NOT semantic similarity - it's based on lexical overlap.
/// Use `FastEmbed` for true semantic understanding.
///
/// # Examples
///
/// ```
/// use rlm_rs::embedding::{Embedder, FallbackEmbedder, DEFAULT_DIMENSIONS};
///
/// let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
/// let emb1 = embedder.embed("hello world").unwrap();
/// let emb2 = embedder.embed("hello world").unwrap();
/// assert_eq!(emb1, emb2); // Deterministic
/// ```
pub struct FallbackEmbedder {
    dimensions: usize,
}

impl FallbackEmbedder {
    /// Creates a new fallback embedder with the specified dimensions.
    #[must_use]
    pub const fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }

    /// Hashes a string to a u64 value.
    fn hash_string(s: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish()
    }

    /// Generates a pseudo-embedding from text.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn generate_embedding(&self, text: &str) -> Vec<f32> {
        let mut embedding = vec![0.0f32; self.dimensions];

        // Normalize text: lowercase and basic cleanup
        let normalized: String = text
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c.is_whitespace() {
                    c.to_ascii_lowercase()
                } else {
                    ' '
                }
            })
            .collect();

        // Split into words
        let words: Vec<&str> = normalized.split_whitespace().collect();

        // Word-level hashing (primary signal)
        for word in &words {
            let hash = Self::hash_string(word);
            let idx = (hash as usize) % self.dimensions;
            // Use hash bits to determine sign and magnitude
            let sign = if (hash >> 32) & 1 == 0 { 1.0 } else { -1.0 };
            let magnitude = 1.0 + ((hash >> 16) & 0xFF) as f32 / 255.0;
            embedding[idx] += sign * magnitude;
        }

        // Character trigram hashing (secondary signal for fuzzy matching)
        let chars: Vec<char> = normalized.chars().collect();
        if chars.len() >= 3 {
            for window in chars.windows(3) {
                let trigram: String = window.iter().collect();
                let hash = Self::hash_string(&trigram);
                let idx = (hash as usize) % self.dimensions;
                let sign = if (hash >> 32) & 1 == 0 { 0.5 } else { -0.5 };
                embedding[idx] += sign;
            }
        }

        // Normalize to unit length for cosine similarity
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for val in &mut embedding {
                *val /= magnitude;
            }
        }

        embedding
    }
}

impl Embedder for FallbackEmbedder {
    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn model_name(&self) -> &'static str {
        "fallback-hash-v1"
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.generate_embedding(text))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Parallel processing for batch embedding
        use rayon::prelude::*;

        Ok(texts
            .par_iter()
            .map(|text| self.generate_embedding(text))
            .collect())
    }
}

// Note: FallbackEmbedder auto-derives Send + Sync because it only contains
// Copy types (usize) with no interior mutability.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{DEFAULT_DIMENSIONS, cosine_similarity};

    #[test]
    fn test_deterministic() {
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
        let emb1 = embedder.embed("hello world").unwrap();
        let emb2 = embedder.embed("hello world").unwrap();
        assert_eq!(emb1, emb2);
    }

    #[test]
    fn test_dimensions() {
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
        let emb = embedder.embed("test").unwrap();
        assert_eq!(emb.len(), DEFAULT_DIMENSIONS);
    }

    #[test]
    fn test_normalized() {
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
        let emb = embedder.embed("hello world").unwrap();
        let magnitude: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_similar_text_higher_similarity() {
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
        let emb_base = embedder.embed("the quick brown fox").unwrap();
        let emb_similar = embedder.embed("the quick brown dog").unwrap();
        let emb_different = embedder.embed("completely unrelated text").unwrap();

        let sim_similar = cosine_similarity(&emb_base, &emb_similar);
        let sim_different = cosine_similarity(&emb_base, &emb_different);

        assert!(
            sim_similar > sim_different,
            "Similar text should have higher similarity: {sim_similar} vs {sim_different}"
        );
    }

    #[test]
    fn test_batch_embedding() {
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
        let texts = vec!["hello", "world", "test"];
        let embeddings = embedder.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for emb in embeddings {
            assert_eq!(emb.len(), DEFAULT_DIMENSIONS);
        }
    }

    #[test]
    fn test_empty_text() {
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);
        let emb = embedder.embed("").unwrap();
        assert_eq!(emb.len(), DEFAULT_DIMENSIONS);
        // Empty text should produce zero vector (all zeros)
        assert!(emb.iter().all(|&x| x == 0.0));
    }
}
