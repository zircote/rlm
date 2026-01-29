//! Hybrid search with semantic and lexical retrieval.
//!
//! Combines vector similarity search with FTS5 BM25 using Reciprocal Rank Fusion (RRF).
//!
//! ## Features
//!
//! - **Semantic Search**: Vector similarity using embeddings
//! - **BM25 Search**: Full-text search using `SQLite` `FTS5`
//! - **Hybrid Search**: Combines both using Reciprocal Rank Fusion
//! - **HNSW Index**: Optional scalable approximate nearest neighbor search (requires `usearch-hnsw` feature)

pub mod hnsw;
mod rrf;

pub use hnsw::{HnswConfig, HnswIndex, HnswResult};
pub use rrf::{RrfConfig, reciprocal_rank_fusion, weighted_rrf};

use crate::embedding::{Embedder, cosine_similarity};
use crate::error::Result;
use crate::storage::{SqliteStorage, Storage};

/// Default similarity threshold for semantic search.
pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.3;

/// Default number of results to return.
pub const DEFAULT_TOP_K: usize = 10;

/// Search result with chunk ID and combined score.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Chunk ID.
    pub chunk_id: i64,
    /// Buffer ID this chunk belongs to.
    pub buffer_id: i64,
    /// Sequential index within the buffer (0-based, for temporal ordering).
    pub index: usize,
    /// Combined RRF score (higher is better).
    pub score: f64,
    /// Semantic similarity score (if available).
    pub semantic_score: Option<f32>,
    /// BM25 score (if available).
    pub bm25_score: Option<f64>,
    /// Content preview (first N characters, if requested).
    pub content_preview: Option<String>,
}

/// Configuration for hybrid search.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum number of results to return.
    pub top_k: usize,
    /// Minimum similarity threshold for semantic results.
    pub similarity_threshold: f32,
    /// RRF k parameter (default 60).
    pub rrf_k: u32,
    /// Whether to include semantic search.
    pub use_semantic: bool,
    /// Whether to include BM25 search.
    pub use_bm25: bool,
    /// Optional buffer ID to scope the search to a single buffer.
    pub buffer_id: Option<i64>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            top_k: DEFAULT_TOP_K,
            similarity_threshold: DEFAULT_SIMILARITY_THRESHOLD,
            rrf_k: 60,
            use_semantic: true,
            use_bm25: true,
            buffer_id: None,
        }
    }
}

/// Default preview length in characters.
pub const DEFAULT_PREVIEW_LEN: usize = 150;

impl SearchResult {
    /// Creates a search result from pre-fetched metadata.
    ///
    /// Use [`build_results_batch`] to construct multiple results efficiently.
    const fn new(
        chunk_id: i64,
        buffer_id: i64,
        index: usize,
        score: f64,
        semantic_score: Option<f32>,
        bm25_score: Option<f64>,
    ) -> Self {
        Self {
            chunk_id,
            buffer_id,
            index,
            score,
            semantic_score,
            bm25_score,
            content_preview: None,
        }
    }
}

/// Batch-constructs search results from scored chunk IDs.
///
/// Performs a single SQL query to fetch `(buffer_id, index)` for all chunk
/// IDs, avoiding the N+1 pattern of per-ID lookups. Chunks that cannot be
/// found in storage are silently omitted.
fn build_results_batch(
    storage: &SqliteStorage,
    scored: &[(i64, f64, Option<f32>, Option<f64>)],
) -> Vec<SearchResult> {
    let ids: Vec<i64> = scored.iter().map(|(id, _, _, _)| *id).collect();
    let Ok(meta) = storage.get_chunk_metadata_batch(&ids) else {
        return Vec::new();
    };

    scored
        .iter()
        .filter_map(|&(chunk_id, score, sem, bm25)| {
            let &(buffer_id, index) = meta.get(&chunk_id)?;
            Some(SearchResult::new(
                chunk_id, buffer_id, index, score, sem, bm25,
            ))
        })
        .collect()
}

/// Populates content previews for search results.
///
/// # Arguments
///
/// * `storage` - The storage backend.
/// * `results` - Search results to populate.
/// * `preview_len` - Maximum preview length in characters.
///
/// # Errors
///
/// Returns an error if chunk retrieval fails.
pub fn populate_previews(
    storage: &SqliteStorage,
    results: &mut [SearchResult],
    preview_len: usize,
) -> Result<()> {
    for result in results.iter_mut() {
        if let Some(chunk) = storage.get_chunk(result.chunk_id)? {
            let content = &chunk.content;
            let preview = if content.len() <= preview_len {
                content.clone()
            } else {
                // Find a valid UTF-8 boundary
                let end = crate::io::find_char_boundary(content, preview_len);
                let mut preview = content[..end].to_string();
                if end < content.len() {
                    preview.push_str("...");
                }
                preview
            };
            result.content_preview = Some(preview);
        }
    }
    Ok(())
}

impl SearchConfig {
    /// Creates a new search config with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the top-k limit.
    #[must_use]
    pub const fn with_top_k(mut self, top_k: usize) -> Self {
        self.top_k = top_k;
        self
    }

    /// Sets the similarity threshold.
    #[must_use]
    pub const fn with_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold;
        self
    }

    /// Sets the RRF k parameter.
    #[must_use]
    pub const fn with_rrf_k(mut self, k: u32) -> Self {
        self.rrf_k = k;
        self
    }

    /// Enables or disables semantic search.
    #[must_use]
    pub const fn with_semantic(mut self, enabled: bool) -> Self {
        self.use_semantic = enabled;
        self
    }

    /// Enables or disables BM25 search.
    #[must_use]
    pub const fn with_bm25(mut self, enabled: bool) -> Self {
        self.use_bm25 = enabled;
        self
    }

    /// Scopes the search to a single buffer.
    #[must_use]
    pub const fn with_buffer_id(mut self, buffer_id: Option<i64>) -> Self {
        self.buffer_id = buffer_id;
        self
    }

    /// Configures semantic/BM25 flags from a mode string.
    ///
    /// Accepted values: `"semantic"`, `"bm25"`, or anything else (treated as
    /// `"hybrid"` — both enabled).
    #[must_use]
    pub fn with_mode(mut self, mode: &str) -> Self {
        match mode {
            "semantic" => {
                self.use_semantic = true;
                self.use_bm25 = false;
            }
            "bm25" => {
                self.use_semantic = false;
                self.use_bm25 = true;
            }
            _ => {
                self.use_semantic = true;
                self.use_bm25 = true;
            }
        }
        self
    }
}

/// Performs hybrid search combining semantic and BM25 results.
///
/// # Arguments
///
/// * `storage` - The storage backend.
/// * `embedder` - The embedding generator.
/// * `query` - The search query text.
/// * `config` - Search configuration.
///
/// # Errors
///
/// Returns an error if search operations fail.
pub fn hybrid_search(
    storage: &SqliteStorage,
    embedder: &dyn Embedder,
    query: &str,
    config: &SearchConfig,
) -> Result<Vec<SearchResult>> {
    let mut semantic_results: Vec<(i64, f32)> = Vec::new();
    let mut bm25_results: Vec<(i64, f64)> = Vec::new();

    // Semantic search
    if config.use_semantic {
        semantic_results = semantic_search(storage, embedder, query, config)?;
    }

    // BM25 search
    if config.use_bm25 {
        bm25_results = if let Some(bid) = config.buffer_id {
            storage.search_fts_buffer(query, bid, config.top_k * 2)?
        } else {
            storage.search_fts(query, config.top_k * 2)?
        };
    }

    // If only one type of search is enabled, return those results directly
    if !config.use_semantic {
        let scored: Vec<_> = bm25_results
            .into_iter()
            .take(config.top_k)
            .map(|(id, score)| (id, score, None, Some(score)))
            .collect();
        return Ok(build_results_batch(storage, &scored));
    }

    if !config.use_bm25 {
        let scored: Vec<_> = semantic_results
            .into_iter()
            .take(config.top_k)
            .map(|(id, score)| (id, f64::from(score), Some(score), None))
            .collect();
        return Ok(build_results_batch(storage, &scored));
    }

    // Combine using RRF
    let rrf_config = RrfConfig::new(config.rrf_k);

    // Convert to ranked lists (already sorted by score descending)
    let semantic_ranked: Vec<i64> = semantic_results.iter().map(|(id, _)| *id).collect();
    let bm25_ranked: Vec<i64> = bm25_results.iter().map(|(id, _)| *id).collect();

    let fused = reciprocal_rank_fusion(&[&semantic_ranked, &bm25_ranked], &rrf_config);

    // Build result with original scores
    let semantic_map: std::collections::HashMap<i64, f32> = semantic_results.into_iter().collect();
    let bm25_map: std::collections::HashMap<i64, f64> = bm25_results.into_iter().collect();

    let scored: Vec<_> = fused
        .into_iter()
        .take(config.top_k)
        .map(|(chunk_id, rrf_score)| {
            (
                chunk_id,
                rrf_score,
                semantic_map.get(&chunk_id).copied(),
                bm25_map.get(&chunk_id).copied(),
            )
        })
        .collect();

    Ok(build_results_batch(storage, &scored))
}

/// Performs semantic similarity search.
///
/// Uses cosine similarity between query embedding and stored chunk embeddings.
fn semantic_search(
    storage: &SqliteStorage,
    embedder: &dyn Embedder,
    query: &str,
    config: &SearchConfig,
) -> Result<Vec<(i64, f32)>> {
    // Generate query embedding
    let query_embedding = embedder.embed(query)?;

    // Get embeddings — scoped to buffer when specified
    let all_embeddings = if let Some(bid) = config.buffer_id {
        storage.get_embeddings_for_buffer(bid)?
    } else {
        storage.get_all_embeddings()?
    };

    if all_embeddings.is_empty() {
        return Ok(Vec::new());
    }

    // Calculate similarities
    let mut similarities: Vec<(i64, f32)> = all_embeddings
        .iter()
        .map(|(chunk_id, embedding)| {
            let sim = cosine_similarity(&query_embedding, embedding);
            (*chunk_id, sim)
        })
        .filter(|(_, sim)| *sim >= config.similarity_threshold)
        .collect();

    // Sort by similarity descending
    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Limit results
    similarities.truncate(config.top_k * 2);

    Ok(similarities)
}

/// Performs semantic-only search.
///
/// # Arguments
///
/// * `storage` - The storage backend.
/// * `embedder` - The embedding generator.
/// * `query` - The search query text.
/// * `top_k` - Maximum number of results.
/// * `threshold` - Minimum similarity threshold.
///
/// # Errors
///
/// Returns an error if search fails.
pub fn search_semantic(
    storage: &SqliteStorage,
    embedder: &dyn Embedder,
    query: &str,
    top_k: usize,
    threshold: f32,
) -> Result<Vec<SearchResult>> {
    let config = SearchConfig::new()
        .with_top_k(top_k)
        .with_threshold(threshold)
        .with_semantic(true)
        .with_bm25(false);

    hybrid_search(storage, embedder, query, &config)
}

/// Performs BM25-only search.
///
/// # Arguments
///
/// * `storage` - The storage backend.
/// * `query` - The search query text.
/// * `top_k` - Maximum number of results.
///
/// # Errors
///
/// Returns an error if search fails.
pub fn search_bm25(
    storage: &SqliteStorage,
    query: &str,
    top_k: usize,
) -> Result<Vec<SearchResult>> {
    let results = storage.search_fts(query, top_k)?;

    let scored: Vec<_> = results
        .into_iter()
        .map(|(id, score)| (id, score, None, Some(score)))
        .collect();
    Ok(build_results_batch(storage, &scored))
}

/// Generates and stores embeddings for all chunks in a buffer.
///
/// # Arguments
///
/// * `storage` - The storage backend (mutable for storing embeddings).
/// * `embedder` - The embedding generator.
/// * `buffer_id` - The buffer ID to process.
///
/// # Returns
///
/// The number of chunks embedded.
///
/// # Errors
///
/// Returns an error if embedding generation or storage fails.
pub fn embed_buffer_chunks(
    storage: &mut SqliteStorage,
    embedder: &dyn Embedder,
    buffer_id: i64,
) -> Result<usize> {
    let chunks = storage.get_chunks(buffer_id)?;

    if chunks.is_empty() {
        return Ok(0);
    }

    // Collect chunk texts for batch embedding
    let texts: Vec<&str> = chunks.iter().map(|c| c.content.as_str()).collect();

    // Generate embeddings in batch
    let embeddings = embedder.embed_batch(&texts)?;

    // Prepare batch for storage
    let batch: Vec<(i64, Vec<f32>)> = chunks
        .iter()
        .zip(embeddings)
        .filter_map(|(chunk, embedding)| chunk.id.map(|id| (id, embedding)))
        .collect();

    let count = batch.len();

    // Store embeddings with model name for version tracking
    storage.store_embeddings_batch(&batch, Some(embedder.model_name()))?;

    Ok(count)
}

/// Checks if a buffer has all chunks embedded.
///
/// # Errors
///
/// Returns an error if the check fails.
pub fn buffer_fully_embedded(storage: &SqliteStorage, buffer_id: i64) -> Result<bool> {
    let chunk_count = storage.chunk_count(buffer_id)?;
    if chunk_count == 0 {
        return Ok(true);
    }

    // Count embeddings for this buffer's chunks
    let chunks = storage.get_chunks(buffer_id)?;
    let mut embedded_count = 0;

    for chunk in &chunks {
        if let Some(id) = chunk.id
            && storage.has_embedding(id)?
        {
            embedded_count += 1;
        }
    }

    Ok(embedded_count == chunk_count)
}

/// Checks if existing embeddings were created with a different model.
///
/// Returns `Some(existing_model)` if there's a model mismatch, `None` otherwise.
///
/// # Errors
///
/// Returns an error if the query fails.
pub fn check_model_mismatch(
    storage: &SqliteStorage,
    buffer_id: i64,
    current_model: &str,
) -> Result<Option<String>> {
    let models = storage.get_embedding_models(buffer_id)?;

    // If no embeddings exist, no mismatch
    if models.is_empty() {
        return Ok(None);
    }

    // Check if any existing model differs from the current one
    for model in models {
        if model != current_model {
            return Ok(Some(model));
        }
    }

    Ok(None)
}

/// Information about embedding model versions for a buffer.
#[derive(Debug, Clone)]
pub struct EmbeddingModelInfo {
    /// Model names and their embedding counts.
    pub models: Vec<(Option<String>, i64)>,
    /// Total number of embeddings.
    pub total_embeddings: i64,
    /// Whether there are mixed model versions.
    pub has_mixed_models: bool,
}

/// Gets embedding model information for a buffer.
///
/// # Errors
///
/// Returns an error if the query fails.
pub fn get_embedding_model_info(
    storage: &SqliteStorage,
    buffer_id: i64,
) -> Result<EmbeddingModelInfo> {
    let models = storage.get_embedding_model_counts(buffer_id)?;
    let total_embeddings: i64 = models.iter().map(|(_, count)| count).sum();
    let distinct_models: std::collections::HashSet<_> =
        models.iter().map(|(name, _)| name.as_deref()).collect();
    let has_mixed_models = distinct_models.len() > 1;

    Ok(EmbeddingModelInfo {
        models,
        total_embeddings,
        has_mixed_models,
    })
}

/// Result of an incremental embedding operation.
#[derive(Debug, Clone)]
pub struct IncrementalEmbedResult {
    /// Number of new embeddings created.
    pub embedded_count: usize,
    /// Number of chunks that were skipped (already embedded with correct model).
    pub skipped_count: usize,
    /// Number of embeddings that were replaced (different model).
    pub replaced_count: usize,
    /// Total chunks in the buffer.
    pub total_chunks: usize,
    /// Model name used for embedding.
    pub model_name: String,
}

impl IncrementalEmbedResult {
    /// Returns true if any embeddings were created or updated.
    #[must_use]
    pub const fn had_changes(&self) -> bool {
        self.embedded_count > 0 || self.replaced_count > 0
    }

    /// Returns the percentage of chunks now embedded.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Acceptable for percentage calculation
    pub fn completion_percentage(&self) -> f64 {
        if self.total_chunks == 0 {
            100.0
        } else {
            let completed = self.embedded_count + self.skipped_count + self.replaced_count;
            (completed as f64 / self.total_chunks as f64) * 100.0
        }
    }
}

/// Incrementally embeds chunks in a buffer.
///
/// Only embeds chunks that:
/// - Have no embedding, OR
/// - Have an embedding from a different model (if `force_reembed` is true)
///
/// This is more efficient than `embed_buffer_chunks` for large buffers
/// where only a few chunks have changed.
///
/// # Arguments
///
/// * `storage` - The storage backend.
/// * `embedder` - The embedder to use.
/// * `buffer_id` - The buffer to embed.
/// * `force_reembed` - If true, re-embeds chunks with different models.
///
/// # Returns
///
/// An `IncrementalEmbedResult` with statistics about what was done.
///
/// # Errors
///
/// Returns an error if embedding generation or storage fails.
pub fn embed_buffer_chunks_incremental(
    storage: &mut SqliteStorage,
    embedder: &dyn Embedder,
    buffer_id: i64,
    force_reembed: bool,
) -> Result<IncrementalEmbedResult> {
    let current_model = embedder.model_name();
    let stats = storage.get_embedding_stats(buffer_id)?;
    let total_chunks = stats.total_chunks;

    // Determine which chunks need embedding
    let model_to_check = if force_reembed {
        Some(current_model)
    } else {
        None
    };

    let chunk_ids_to_embed = storage.get_chunks_needing_embedding(buffer_id, model_to_check)?;

    if chunk_ids_to_embed.is_empty() {
        return Ok(IncrementalEmbedResult {
            embedded_count: 0,
            skipped_count: total_chunks,
            replaced_count: 0,
            total_chunks,
            model_name: current_model.to_string(),
        });
    }

    // Load the chunks we need to embed
    let all_chunks = storage.get_chunks(buffer_id)?;
    let chunks_to_embed: Vec<_> = all_chunks
        .iter()
        .filter(|c| c.id.is_some_and(|id| chunk_ids_to_embed.contains(&id)))
        .collect();

    // Count how many are replacements (had embeddings before)
    let mut replaced_count = 0;
    for chunk in &chunks_to_embed {
        if let Some(id) = chunk.id
            && storage.has_embedding(id)?
        {
            replaced_count += 1;
        }
    }

    // Generate embeddings
    let texts: Vec<&str> = chunks_to_embed.iter().map(|c| c.content.as_str()).collect();
    let embeddings = embedder.embed_batch(&texts)?;

    // Store embeddings
    let batch: Vec<(i64, Vec<f32>)> = chunks_to_embed
        .iter()
        .zip(embeddings)
        .filter_map(|(chunk, embedding)| chunk.id.map(|id| (id, embedding)))
        .collect();

    let embedded_count = batch.len();
    storage.store_embeddings_batch(&batch, Some(current_model))?;

    let new_embeddings = embedded_count - replaced_count;
    let skipped_count = total_chunks - embedded_count;

    Ok(IncrementalEmbedResult {
        embedded_count: new_embeddings,
        skipped_count,
        replaced_count,
        total_chunks,
        model_name: current_model.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Buffer, Chunk};
    use crate::embedding::{DEFAULT_DIMENSIONS, FallbackEmbedder};
    use crate::storage::Storage;

    fn setup_storage() -> SqliteStorage {
        let mut storage = SqliteStorage::in_memory().unwrap();
        storage.init().unwrap();
        storage
    }

    fn setup_storage_with_chunks() -> SqliteStorage {
        let mut storage = setup_storage();

        // Create a buffer
        let buffer = Buffer::from_named(
            "test.txt".to_string(),
            "Test content for searching".to_string(),
        );
        let buffer_id = storage.add_buffer(&buffer).unwrap();

        // Create chunks with different content
        let chunks = vec![
            Chunk::new(
                buffer_id,
                "The quick brown fox jumps over the lazy dog".to_string(),
                0..44,
                0,
            ),
            Chunk::new(
                buffer_id,
                "Machine learning is a subset of artificial intelligence".to_string(),
                44..100,
                1,
            ),
            Chunk::new(
                buffer_id,
                "Rust is a systems programming language".to_string(),
                100..139,
                2,
            ),
        ];

        storage.add_chunks(buffer_id, &chunks).unwrap();

        storage
    }

    #[test]
    fn test_search_config_default() {
        let config = SearchConfig::default();
        assert_eq!(config.top_k, DEFAULT_TOP_K);
        assert!((config.similarity_threshold - DEFAULT_SIMILARITY_THRESHOLD).abs() < f32::EPSILON);
        assert_eq!(config.rrf_k, 60);
        assert!(config.use_semantic);
        assert!(config.use_bm25);
    }

    #[test]
    fn test_search_config_builder() {
        let config = SearchConfig::new()
            .with_top_k(20)
            .with_threshold(0.5)
            .with_rrf_k(30)
            .with_semantic(false)
            .with_bm25(true);

        assert_eq!(config.top_k, 20);
        assert!((config.similarity_threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.rrf_k, 30);
        assert!(!config.use_semantic);
        assert!(config.use_bm25);
    }

    #[test]
    fn test_search_bm25() {
        let storage = setup_storage_with_chunks();

        // Search for "fox" - should find the first chunk
        let results = search_bm25(&storage, "fox", 10).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].bm25_score.is_some());
        assert!(results[0].semantic_score.is_none());
    }

    #[test]
    fn test_search_bm25_no_results() {
        let storage = setup_storage_with_chunks();

        // Search for something not in the content
        let results = search_bm25(&storage, "xyz123nonexistent", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_embed_buffer_chunks() {
        let mut storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // Embed chunks for buffer 1
        let count = embed_buffer_chunks(&mut storage, &embedder, 1).unwrap();
        assert_eq!(count, 3); // We created 3 chunks
    }

    #[test]
    fn test_embed_buffer_chunks_empty() {
        let mut storage = setup_storage();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // Create buffer with no chunks
        let buffer = Buffer::from_named("empty.txt".to_string(), String::new());
        let buffer_id = storage.add_buffer(&buffer).unwrap();

        let count = embed_buffer_chunks(&mut storage, &embedder, buffer_id).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_buffer_fully_embedded_empty() {
        let mut storage = setup_storage();

        // Create buffer with no chunks
        let buffer = Buffer::from_named("empty.txt".to_string(), String::new());
        let buffer_id = storage.add_buffer(&buffer).unwrap();

        // Empty buffer should be "fully embedded"
        let result = buffer_fully_embedded(&storage, buffer_id).unwrap();
        assert!(result);
    }

    #[test]
    fn test_buffer_fully_embedded_with_embeddings() {
        let mut storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // Before embedding
        let result = buffer_fully_embedded(&storage, 1).unwrap();
        assert!(!result);

        // Embed all chunks
        embed_buffer_chunks(&mut storage, &embedder, 1).unwrap();

        // After embedding
        let result = buffer_fully_embedded(&storage, 1).unwrap();
        assert!(result);
    }

    #[test]
    fn test_hybrid_search_bm25_only() {
        let storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        let config = SearchConfig::new().with_semantic(false).with_bm25(true);

        let results = hybrid_search(&storage, &embedder, "programming", &config).unwrap();
        // Should find "Rust is a systems programming language"
        assert!(!results.is_empty());
        assert!(results[0].bm25_score.is_some());
        assert!(results[0].semantic_score.is_none());
    }

    #[test]
    fn test_hybrid_search_semantic_only() {
        let mut storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // First embed the chunks
        embed_buffer_chunks(&mut storage, &embedder, 1).unwrap();

        let config = SearchConfig::new()
            .with_semantic(true)
            .with_bm25(false)
            .with_threshold(0.0); // Low threshold for fallback embedder

        let results = hybrid_search(&storage, &embedder, "programming language", &config).unwrap();
        assert!(!results.is_empty());
        assert!(results[0].semantic_score.is_some());
        assert!(results[0].bm25_score.is_none());
    }

    #[test]
    fn test_hybrid_search_both() {
        let mut storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // First embed the chunks
        embed_buffer_chunks(&mut storage, &embedder, 1).unwrap();

        let config = SearchConfig::new()
            .with_semantic(true)
            .with_bm25(true)
            .with_threshold(0.0); // Low threshold for fallback embedder

        let results = hybrid_search(&storage, &embedder, "programming", &config).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_semantic() {
        let mut storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // First embed the chunks
        embed_buffer_chunks(&mut storage, &embedder, 1).unwrap();

        let results = search_semantic(&storage, &embedder, "test query", 10, 0.0).unwrap();
        // Should return results with semantic scores only
        for result in &results {
            assert!(result.semantic_score.is_some());
            assert!(result.bm25_score.is_none());
        }
    }

    #[test]
    fn test_search_semantic_empty_embeddings() {
        let storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // Don't embed chunks - search should return empty
        let results = search_semantic(&storage, &embedder, "test query", 10, 0.5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_incremental_embed_new_chunks() {
        let mut storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // First incremental embed - should embed all 3 chunks
        let result = embed_buffer_chunks_incremental(&mut storage, &embedder, 1, false).unwrap();
        assert_eq!(result.embedded_count, 3);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.replaced_count, 0);
        assert_eq!(result.total_chunks, 3);
        assert!(result.had_changes());

        // Second incremental embed - should skip all (already embedded)
        let result2 = embed_buffer_chunks_incremental(&mut storage, &embedder, 1, false).unwrap();
        assert_eq!(result2.embedded_count, 0);
        assert_eq!(result2.skipped_count, 3);
        assert_eq!(result2.replaced_count, 0);
        assert!(!result2.had_changes());
    }

    #[test]
    fn test_incremental_embed_force_reembed() {
        let mut storage = setup_storage_with_chunks();
        let embedder = FallbackEmbedder::new(DEFAULT_DIMENSIONS);

        // First embed normally
        embed_buffer_chunks_incremental(&mut storage, &embedder, 1, false).unwrap();

        // Force re-embed - should replace all 3
        let result = embed_buffer_chunks_incremental(&mut storage, &embedder, 1, true).unwrap();
        // All chunks already have correct model, so no changes needed even with force
        // (force only affects different-model embeddings)
        assert_eq!(result.skipped_count, 3);
        assert!(!result.had_changes());
    }

    #[test]
    fn test_incremental_embed_result_completion() {
        let result = IncrementalEmbedResult {
            embedded_count: 2,
            skipped_count: 3,
            replaced_count: 0,
            total_chunks: 5,
            model_name: "test".to_string(),
        };
        assert!(result.had_changes());
        assert!((result.completion_percentage() - 100.0).abs() < f64::EPSILON);
    }
}
