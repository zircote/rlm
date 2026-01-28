//! Storage trait definition.
//!
//! Defines the interface for persistent storage backends, enabling
//! pluggable storage implementations.

use crate::core::{Buffer, Chunk, Context};
use crate::error::Result;
use serde::Serialize;

/// Trait for persistent storage backends.
///
/// Implementations handle storage of RLM state including contexts,
/// buffers, and chunks. All operations should be atomic where appropriate.
///
/// Note: This trait does not require `Send + Sync` as rlm-rs is a
/// single-threaded CLI application. Implementations are not guaranteed
/// to be thread-safe.
pub trait Storage {
    /// Initializes storage (creates schema, runs migrations).
    ///
    /// Should be idempotent - safe to call multiple times.
    ///
    /// # Errors
    ///
    /// Returns an error if schema creation or migration fails.
    fn init(&mut self) -> Result<()>;

    /// Checks if storage is initialized.
    ///
    /// # Errors
    ///
    /// Returns an error if the check cannot be performed.
    fn is_initialized(&self) -> Result<bool>;

    /// Resets all stored state.
    ///
    /// Deletes all data but preserves the schema.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    fn reset(&mut self) -> Result<()>;

    // ==================== Context Operations ====================

    /// Saves the current context state.
    ///
    /// Creates or updates the context in storage.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization or database write fails.
    fn save_context(&mut self, context: &Context) -> Result<()>;

    /// Loads the context state.
    ///
    /// Returns `None` if no context exists.
    ///
    /// # Errors
    ///
    /// Returns an error if database read or deserialization fails.
    fn load_context(&self) -> Result<Option<Context>>;

    /// Deletes the current context.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    fn delete_context(&mut self) -> Result<()>;

    // ==================== Buffer Operations ====================

    /// Adds a buffer to storage.
    ///
    /// Returns the assigned buffer ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer cannot be inserted.
    fn add_buffer(&mut self, buffer: &Buffer) -> Result<i64>;

    /// Retrieves a buffer by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn get_buffer(&self, id: i64) -> Result<Option<Buffer>>;

    /// Retrieves a buffer by name.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn get_buffer_by_name(&self, name: &str) -> Result<Option<Buffer>>;

    /// Lists all buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn list_buffers(&self) -> Result<Vec<Buffer>>;

    /// Updates an existing buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer does not exist or update fails.
    fn update_buffer(&mut self, buffer: &Buffer) -> Result<()>;

    /// Deletes a buffer by ID.
    ///
    /// Also deletes associated chunks.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    fn delete_buffer(&mut self, id: i64) -> Result<()>;

    /// Returns the count of buffers.
    ///
    /// # Errors
    ///
    /// Returns an error if the count query fails.
    fn buffer_count(&self) -> Result<usize>;

    // ==================== Chunk Operations ====================

    /// Adds chunks for a buffer.
    ///
    /// Should be called after buffer is created.
    ///
    /// # Errors
    ///
    /// Returns an error if chunk insertion fails.
    fn add_chunks(&mut self, buffer_id: i64, chunks: &[Chunk]) -> Result<()>;

    /// Retrieves all chunks for a buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn get_chunks(&self, buffer_id: i64) -> Result<Vec<Chunk>>;

    /// Retrieves a specific chunk by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    fn get_chunk(&self, id: i64) -> Result<Option<Chunk>>;

    /// Deletes all chunks for a buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    fn delete_chunks(&mut self, buffer_id: i64) -> Result<()>;

    /// Returns the count of chunks for a buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if the count query fails.
    fn chunk_count(&self, buffer_id: i64) -> Result<usize>;

    // ==================== Utility Operations ====================

    /// Exports all buffers as a concatenated string.
    ///
    /// Used for the `export-buffers` command.
    ///
    /// # Errors
    ///
    /// Returns an error if buffer retrieval fails.
    fn export_buffers(&self) -> Result<String>;

    /// Gets storage statistics.
    ///
    /// # Errors
    ///
    /// Returns an error if statistics cannot be gathered.
    fn stats(&self) -> Result<StorageStats>;
}

/// Storage statistics.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StorageStats {
    /// Number of buffers stored.
    pub buffer_count: usize,
    /// Total number of chunks across all buffers.
    pub chunk_count: usize,
    /// Total size of all buffer content in bytes.
    pub total_content_size: usize,
    /// Whether a context is stored.
    pub has_context: bool,
    /// Schema version.
    pub schema_version: u32,
    /// Database file size in bytes (if applicable).
    pub db_size: Option<u64>,
}

/// Trait for vector-based semantic search (feature-gated).
#[cfg(feature = "usearch-hnsw")]
pub trait VectorStorage: Storage {
    /// Indexes a chunk with embeddings for semantic search.
    ///
    /// # Errors
    ///
    /// Returns an error if indexing fails.
    fn index_chunk(&mut self, chunk_id: i64, embedding: &[f32]) -> Result<()>;

    /// Performs semantic search for similar chunks.
    ///
    /// Returns chunk IDs and similarity scores.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    fn search_similar(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<(i64, f32)>>;

    /// Removes vector index for a chunk.
    ///
    /// # Errors
    ///
    /// Returns an error if removal fails.
    fn remove_index(&mut self, chunk_id: i64) -> Result<()>;
}
