//! `SQLite` storage implementation.
//!
//! Provides persistent storage using `SQLite` with proper transaction
//! management and migration support.

// SQLite stores all integers as i64. These casts are intentional and safe
// because we only store non-negative values that fit in usize.
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]

use crate::core::{Buffer, BufferMetadata, Chunk, ChunkMetadata, Context};
use crate::error::{Result, StorageError};
use crate::storage::schema::{
    CHECK_SCHEMA_SQL, CURRENT_SCHEMA_VERSION, GET_VERSION_SQL, SCHEMA_SQL, SET_VERSION_SQL,
};
use crate::storage::traits::{Storage, StorageStats};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};

/// SQLite-based storage implementation.
///
/// Provides persistent storage for RLM state with full ACID guarantees.
///
/// # Examples
///
/// ```no_run
/// use rlm_rs::storage::{SqliteStorage, Storage};
///
/// let mut storage = SqliteStorage::open("rlm-state.db").unwrap();
/// storage.init().unwrap();
/// ```
pub struct SqliteStorage {
    /// `SQLite` connection.
    conn: Connection,
    /// Path to the database file (None for in-memory).
    path: Option<PathBuf>,
}

impl SqliteStorage {
    /// Opens or creates a `SQLite` database at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the database file. Parent directory must exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or initialized.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| StorageError::Database(e.to_string()))?;
            }
        }

        let conn = Connection::open(&path).map_err(StorageError::from)?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON;", [])
            .map_err(StorageError::from)?;

        // Use WAL mode for better concurrent access (returns result, use query_row)
        let _: String = conn
            .query_row("PRAGMA journal_mode = WAL;", [], |row| row.get(0))
            .map_err(StorageError::from)?;

        Ok(Self {
            conn,
            path: Some(path),
        })
    }

    /// Creates an in-memory `SQLite` database.
    ///
    /// Useful for testing.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be created.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(StorageError::from)?;
        conn.execute("PRAGMA foreign_keys = ON;", [])
            .map_err(StorageError::from)?;

        Ok(Self { conn, path: None })
    }

    /// Returns the database path (None for in-memory).
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Gets the current schema version.
    fn get_schema_version(&self) -> Result<Option<u32>> {
        let version: Option<String> = self
            .conn
            .query_row(GET_VERSION_SQL, [], |row| row.get(0))
            .optional()
            .map_err(StorageError::from)?;

        Ok(version.and_then(|v| v.parse().ok()))
    }

    /// Sets the schema version.
    fn set_schema_version(&self, version: u32) -> Result<()> {
        self.conn
            .execute(SET_VERSION_SQL, params![version.to_string()])
            .map_err(StorageError::from)?;
        Ok(())
    }

    /// Returns current Unix timestamp.
    #[allow(clippy::cast_possible_wrap)]
    fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

impl Storage for SqliteStorage {
    fn init(&mut self) -> Result<()> {
        // Check if already initialized
        let is_init: i64 = self
            .conn
            .query_row(CHECK_SCHEMA_SQL, [], |row| row.get(0))
            .map_err(StorageError::from)?;

        if is_init == 0 {
            // Fresh install - create schema
            self.conn
                .execute_batch(SCHEMA_SQL)
                .map_err(StorageError::from)?;
            self.set_schema_version(CURRENT_SCHEMA_VERSION)?;
        } else {
            // Check for migrations
            if let Some(current) = self.get_schema_version()? {
                if current < CURRENT_SCHEMA_VERSION {
                    // Run migrations
                    let migrations = crate::storage::schema::get_migrations_from(current);
                    for migration in migrations {
                        self.conn
                            .execute_batch(migration.sql)
                            .map_err(|e| StorageError::Migration(e.to_string()))?;
                    }
                    self.set_schema_version(CURRENT_SCHEMA_VERSION)?;
                }
            }
        }

        Ok(())
    }

    fn is_initialized(&self) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row(CHECK_SCHEMA_SQL, [], |row| row.get(0))
            .map_err(StorageError::from)?;
        Ok(count > 0)
    }

    fn reset(&mut self) -> Result<()> {
        self.conn
            .execute_batch(
                r"
            DELETE FROM chunk_embeddings;
            DELETE FROM chunks;
            DELETE FROM buffers;
            DELETE FROM context;
            DELETE FROM metadata;
        ",
            )
            .map_err(StorageError::from)?;
        Ok(())
    }

    // ==================== Context Operations ====================

    fn save_context(&mut self, context: &Context) -> Result<()> {
        let data = serde_json::to_string(context).map_err(StorageError::from)?;
        let now = Self::now();

        self.conn
            .execute(
                r"
            INSERT OR REPLACE INTO context (id, data, created_at, updated_at)
            VALUES (1, ?, COALESCE((SELECT created_at FROM context WHERE id = 1), ?), ?)
        ",
                params![data, now, now],
            )
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn load_context(&self) -> Result<Option<Context>> {
        let data: Option<String> = self
            .conn
            .query_row("SELECT data FROM context WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .map_err(StorageError::from)?;

        match data {
            Some(json) => {
                let context = serde_json::from_str(&json).map_err(StorageError::from)?;
                Ok(Some(context))
            }
            None => Ok(None),
        }
    }

    fn delete_context(&mut self) -> Result<()> {
        self.conn
            .execute("DELETE FROM context WHERE id = 1", [])
            .map_err(StorageError::from)?;
        Ok(())
    }

    // ==================== Buffer Operations ====================

    #[allow(clippy::cast_possible_wrap)]
    fn add_buffer(&mut self, buffer: &Buffer) -> Result<i64> {
        let now = Self::now();

        self.conn
            .execute(
                r"
            INSERT INTO buffers (
                name, source_path, content, content_type, content_hash,
                size, line_count, chunk_count, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ",
                params![
                    buffer.name,
                    buffer
                        .source
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
                    buffer.content,
                    buffer.metadata.content_type,
                    buffer.metadata.content_hash,
                    buffer.metadata.size as i64,
                    buffer.metadata.line_count.map(|c| c as i64),
                    buffer.metadata.chunk_count.map(|c| c as i64),
                    now,
                    now,
                ],
            )
            .map_err(StorageError::from)?;

        Ok(self.conn.last_insert_rowid())
    }

    fn get_buffer(&self, id: i64) -> Result<Option<Buffer>> {
        let result = self
            .conn
            .query_row(
                r"
            SELECT id, name, source_path, content, content_type, content_hash,
                   size, line_count, chunk_count, created_at, updated_at
            FROM buffers WHERE id = ?
        ",
                params![id],
                |row| {
                    Ok(Buffer {
                        id: Some(row.get::<_, i64>(0)?),
                        name: row.get(1)?,
                        source: row.get::<_, Option<String>>(2)?.map(PathBuf::from),
                        content: row.get(3)?,
                        metadata: BufferMetadata {
                            content_type: row.get(4)?,
                            content_hash: row.get(5)?,
                            size: row.get::<_, i64>(6)? as usize,
                            line_count: row.get::<_, Option<i64>>(7)?.map(|c| c as usize),
                            chunk_count: row.get::<_, Option<i64>>(8)?.map(|c| c as usize),
                            created_at: row.get(9)?,
                            updated_at: row.get(10)?,
                        },
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)?;

        Ok(result)
    }

    fn get_buffer_by_name(&self, name: &str) -> Result<Option<Buffer>> {
        let id: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM buffers WHERE name = ?",
                params![name],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::from)?;

        id.map_or(Ok(None), |id| self.get_buffer(id))
    }

    fn list_buffers(&self) -> Result<Vec<Buffer>> {
        let mut stmt = self
            .conn
            .prepare(
                r"
            SELECT id, name, source_path, content, content_type, content_hash,
                   size, line_count, chunk_count, created_at, updated_at
            FROM buffers ORDER BY id
        ",
            )
            .map_err(StorageError::from)?;

        let buffers = stmt
            .query_map([], |row| {
                Ok(Buffer {
                    id: Some(row.get::<_, i64>(0)?),
                    name: row.get(1)?,
                    source: row.get::<_, Option<String>>(2)?.map(PathBuf::from),
                    content: row.get(3)?,
                    metadata: BufferMetadata {
                        content_type: row.get(4)?,
                        content_hash: row.get(5)?,
                        size: row.get::<_, i64>(6)? as usize,
                        line_count: row.get::<_, Option<i64>>(7)?.map(|c| c as usize),
                        chunk_count: row.get::<_, Option<i64>>(8)?.map(|c| c as usize),
                        created_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    },
                })
            })
            .map_err(StorageError::from)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)?;

        Ok(buffers)
    }

    #[allow(clippy::cast_possible_wrap)]
    fn update_buffer(&mut self, buffer: &Buffer) -> Result<()> {
        let id = buffer.id.ok_or_else(|| StorageError::BufferNotFound {
            identifier: "no ID".to_string(),
        })?;

        let now = Self::now();

        self.conn
            .execute(
                r"
            UPDATE buffers SET
                name = ?, source_path = ?, content = ?, content_type = ?,
                content_hash = ?, size = ?, line_count = ?, chunk_count = ?,
                updated_at = ?
            WHERE id = ?
        ",
                params![
                    buffer.name,
                    buffer
                        .source
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
                    buffer.content,
                    buffer.metadata.content_type,
                    buffer.metadata.content_hash,
                    buffer.metadata.size as i64,
                    buffer.metadata.line_count.map(|c| c as i64),
                    buffer.metadata.chunk_count.map(|c| c as i64),
                    now,
                    id,
                ],
            )
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn delete_buffer(&mut self, id: i64) -> Result<()> {
        // Chunks are deleted automatically via CASCADE
        self.conn
            .execute("DELETE FROM buffers WHERE id = ?", params![id])
            .map_err(StorageError::from)?;
        Ok(())
    }

    fn buffer_count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM buffers", [], |row| row.get(0))
            .map_err(StorageError::from)?;
        Ok(count as usize)
    }

    // ==================== Chunk Operations ====================

    #[allow(clippy::cast_possible_wrap)]
    fn add_chunks(&mut self, buffer_id: i64, chunks: &[Chunk]) -> Result<()> {
        let tx = self.conn.transaction().map_err(StorageError::from)?;
        let now = Self::now();

        {
            let mut stmt = tx
                .prepare(
                    r"
                INSERT INTO chunks (
                    buffer_id, content, byte_start, byte_end, chunk_index,
                    strategy, token_count, line_start, line_end, has_overlap,
                    content_hash, custom_metadata, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
                )
                .map_err(StorageError::from)?;

            for chunk in chunks {
                let custom_meta = chunk.metadata.custom.clone();

                let (line_start, line_end) = chunk
                    .metadata
                    .line_range
                    .as_ref()
                    .map_or((None, None), |r| (Some(r.start as i64), Some(r.end as i64)));

                stmt.execute(params![
                    buffer_id,
                    chunk.content,
                    chunk.byte_range.start as i64,
                    chunk.byte_range.end as i64,
                    chunk.index as i64,
                    chunk.metadata.strategy,
                    chunk.metadata.token_count.map(|c| c as i64),
                    line_start,
                    line_end,
                    i64::from(chunk.metadata.has_overlap),
                    chunk.metadata.content_hash,
                    custom_meta,
                    now,
                ])
                .map_err(StorageError::from)?;
            }
        }

        tx.commit().map_err(StorageError::from)?;

        // Update chunk count on buffer
        self.conn
            .execute(
                "UPDATE buffers SET chunk_count = ? WHERE id = ?",
                params![chunks.len() as i64, buffer_id],
            )
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn get_chunks(&self, buffer_id: i64) -> Result<Vec<Chunk>> {
        let mut stmt = self
            .conn
            .prepare(
                r"
            SELECT id, buffer_id, content, byte_start, byte_end, chunk_index,
                   strategy, token_count, line_start, line_end, has_overlap,
                   content_hash, custom_metadata, created_at
            FROM chunks WHERE buffer_id = ? ORDER BY chunk_index
        ",
            )
            .map_err(StorageError::from)?;

        let chunks = stmt
            .query_map(params![buffer_id], |row| {
                let line_start: Option<i64> = row.get(8)?;
                let line_end: Option<i64> = row.get(9)?;
                let line_range = match (line_start, line_end) {
                    (Some(s), Some(e)) => Some((s as usize)..(e as usize)),
                    _ => None,
                };

                Ok(Chunk {
                    id: Some(row.get::<_, i64>(0)?),
                    buffer_id: row.get(1)?,
                    content: row.get(2)?,
                    byte_range: (row.get::<_, i64>(3)? as usize)..(row.get::<_, i64>(4)? as usize),
                    index: row.get::<_, i64>(5)? as usize,
                    metadata: ChunkMetadata {
                        strategy: row.get(6)?,
                        token_count: row.get::<_, Option<i64>>(7)?.map(|c| c as usize),
                        line_range,
                        has_overlap: row.get::<_, i64>(10)? != 0,
                        content_hash: row.get(11)?,
                        custom: row.get(12)?,
                        created_at: row.get(13)?,
                    },
                })
            })
            .map_err(StorageError::from)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)?;

        Ok(chunks)
    }

    fn get_chunk(&self, id: i64) -> Result<Option<Chunk>> {
        let result = self
            .conn
            .query_row(
                r"
            SELECT id, buffer_id, content, byte_start, byte_end, chunk_index,
                   strategy, token_count, line_start, line_end, has_overlap,
                   content_hash, custom_metadata, created_at
            FROM chunks WHERE id = ?
        ",
                params![id],
                |row| {
                    let line_start: Option<i64> = row.get(8)?;
                    let line_end: Option<i64> = row.get(9)?;
                    let line_range = match (line_start, line_end) {
                        (Some(s), Some(e)) => Some((s as usize)..(e as usize)),
                        _ => None,
                    };

                    Ok(Chunk {
                        id: Some(row.get::<_, i64>(0)?),
                        buffer_id: row.get(1)?,
                        content: row.get(2)?,
                        byte_range: (row.get::<_, i64>(3)? as usize)
                            ..(row.get::<_, i64>(4)? as usize),
                        index: row.get::<_, i64>(5)? as usize,
                        metadata: ChunkMetadata {
                            strategy: row.get(6)?,
                            token_count: row.get::<_, Option<i64>>(7)?.map(|c| c as usize),
                            line_range,
                            has_overlap: row.get::<_, i64>(10)? != 0,
                            content_hash: row.get(11)?,
                            custom: row.get(12)?,
                            created_at: row.get(13)?,
                        },
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)?;

        Ok(result)
    }

    fn delete_chunks(&mut self, buffer_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM chunks WHERE buffer_id = ?", params![buffer_id])
            .map_err(StorageError::from)?;

        // Update chunk count on buffer
        self.conn
            .execute(
                "UPDATE buffers SET chunk_count = 0 WHERE id = ?",
                params![buffer_id],
            )
            .map_err(StorageError::from)?;

        Ok(())
    }

    fn chunk_count(&self, buffer_id: i64) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM chunks WHERE buffer_id = ?",
                params![buffer_id],
                |row| row.get(0),
            )
            .map_err(StorageError::from)?;
        Ok(count as usize)
    }

    // ==================== Utility Operations ====================

    fn export_buffers(&self) -> Result<String> {
        let buffers = self.list_buffers()?;
        let mut output = String::new();

        for (i, buffer) in buffers.iter().enumerate() {
            if i > 0 {
                output.push_str("\n\n");
            }
            output.push_str(&buffer.content);
        }

        Ok(output)
    }

    fn stats(&self) -> Result<StorageStats> {
        let buffer_count = self.buffer_count()?;

        let chunk_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .map_err(StorageError::from)?;

        let total_size: i64 = self
            .conn
            .query_row("SELECT COALESCE(SUM(size), 0) FROM buffers", [], |row| {
                row.get(0)
            })
            .map_err(StorageError::from)?;

        let has_context = self.load_context()?.is_some();

        let schema_version = self.get_schema_version()?.unwrap_or(0);

        let db_size = self
            .path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok().map(|m| m.len()));

        Ok(StorageStats {
            buffer_count,
            chunk_count: chunk_count as usize,
            total_content_size: total_size as usize,
            has_context,
            schema_version,
            db_size,
        })
    }
}

// ==================== Embedding & Search Operations ====================

impl SqliteStorage {
    /// Stores an embedding for a chunk.
    ///
    /// # Arguments
    ///
    /// * `chunk_id` - The chunk ID to associate the embedding with.
    /// * `embedding` - The embedding vector (f32 array).
    /// * `model_name` - Optional name of the model that generated the embedding.
    ///
    /// # Errors
    ///
    /// Returns an error if the embedding cannot be stored.
    #[allow(clippy::cast_possible_wrap)]
    pub fn store_embedding(
        &mut self,
        chunk_id: i64,
        embedding: &[f32],
        model_name: Option<&str>,
    ) -> Result<()> {
        let now = Self::now();

        // Serialize f32 array to bytes (little-endian)
        let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        self.conn
            .execute(
                r"
                INSERT OR REPLACE INTO chunk_embeddings (chunk_id, embedding, dimensions, model_name, created_at)
                VALUES (?, ?, ?, ?, ?)
            ",
                params![chunk_id, bytes, embedding.len() as i64, model_name, now],
            )
            .map_err(StorageError::from)?;

        Ok(())
    }

    /// Retrieves the embedding for a chunk.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_embedding(&self, chunk_id: i64) -> Result<Option<Vec<f32>>> {
        let result: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT embedding FROM chunk_embeddings WHERE chunk_id = ?",
                params![chunk_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(|bytes| {
            bytes
                .chunks_exact(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect()
        }))
    }

    /// Stores embeddings for multiple chunks in a batch.
    ///
    /// # Errors
    ///
    /// Returns an error if any embedding cannot be stored.
    #[allow(clippy::cast_possible_wrap)]
    pub fn store_embeddings_batch(
        &mut self,
        embeddings: &[(i64, Vec<f32>)],
        model_name: Option<&str>,
    ) -> Result<()> {
        let tx = self.conn.transaction().map_err(StorageError::from)?;
        let now = Self::now();

        {
            let mut stmt = tx
                .prepare(
                    r"
                    INSERT OR REPLACE INTO chunk_embeddings (chunk_id, embedding, dimensions, model_name, created_at)
                    VALUES (?, ?, ?, ?, ?)
                ",
                )
                .map_err(StorageError::from)?;

            for (chunk_id, embedding) in embeddings {
                let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

                stmt.execute(params![
                    chunk_id,
                    bytes,
                    embedding.len() as i64,
                    model_name,
                    now
                ])
                .map_err(StorageError::from)?;
            }
        }

        tx.commit().map_err(StorageError::from)?;
        Ok(())
    }

    /// Deletes the embedding for a chunk.
    ///
    /// # Errors
    ///
    /// Returns an error if deletion fails.
    pub fn delete_embedding(&mut self, chunk_id: i64) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM chunk_embeddings WHERE chunk_id = ?",
                params![chunk_id],
            )
            .map_err(StorageError::from)?;
        Ok(())
    }

    /// Performs FTS5 BM25 full-text search.
    ///
    /// Returns chunk IDs and their BM25 scores (lower is better match).
    ///
    /// # Arguments
    ///
    /// * `query` - The search query (supports FTS5 query syntax).
    /// * `limit` - Maximum number of results to return.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    #[allow(clippy::cast_possible_wrap)]
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<(i64, f64)>> {
        // FTS5 bm25() returns negative scores, more negative = better match
        // We negate it so higher scores = better match

        // Convert space-separated terms to OR query for more forgiving search
        // "CLI tool" becomes "CLI OR tool" so either term matches
        let fts_query = query
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" OR ");

        let mut stmt = self
            .conn
            .prepare(
                r"
                SELECT rowid, -bm25(chunks_fts) as score
                FROM chunks_fts
                WHERE chunks_fts MATCH ?
                ORDER BY score DESC
                LIMIT ?
            ",
            )
            .map_err(StorageError::from)?;

        let results = stmt
            .query_map(params![fts_query, limit as i64], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
            })
            .map_err(StorageError::from)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)?;

        Ok(results)
    }

    /// Returns all chunk embeddings for vector similarity search.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn get_all_embeddings(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT chunk_id, embedding FROM chunk_embeddings")
            .map_err(StorageError::from)?;

        let results = stmt
            .query_map([], |row| {
                let chunk_id: i64 = row.get(0)?;
                let bytes: Vec<u8> = row.get(1)?;
                let embedding: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                    .collect();
                Ok((chunk_id, embedding))
            })
            .map_err(StorageError::from)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(StorageError::from)?;

        Ok(results)
    }

    /// Counts chunks with embeddings.
    ///
    /// # Errors
    ///
    /// Returns an error if the count fails.
    pub fn embedding_count(&self) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |row| {
                row.get(0)
            })
            .map_err(StorageError::from)?;
        Ok(count as usize)
    }

    /// Checks if a chunk has an embedding.
    ///
    /// # Errors
    ///
    /// Returns an error if the query fails.
    pub fn has_embedding(&self, chunk_id: i64) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM chunk_embeddings WHERE chunk_id = ?",
                params![chunk_id],
                |row| row.get(0),
            )
            .map_err(StorageError::from)?;
        Ok(count > 0)
    }
}

// SAFETY: SqliteStorage is only accessed from a single thread at a time.
// The Storage trait requires Send + Sync, and we ensure thread-safety
// through external synchronization (single-threaded CLI usage).
#[allow(unsafe_code)]
unsafe impl Send for SqliteStorage {}
#[allow(unsafe_code)]
unsafe impl Sync for SqliteStorage {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ContextValue;

    fn setup() -> SqliteStorage {
        let mut storage = SqliteStorage::in_memory().unwrap();
        storage.init().unwrap();
        storage
    }

    #[test]
    fn test_init() {
        let mut storage = SqliteStorage::in_memory().unwrap();
        assert!(storage.init().is_ok());
        assert!(storage.is_initialized().unwrap());
    }

    #[test]
    fn test_init_idempotent() {
        let mut storage = SqliteStorage::in_memory().unwrap();
        assert!(storage.init().is_ok());
        assert!(storage.init().is_ok()); // Second init should be fine
    }

    #[test]
    fn test_context_crud() {
        let mut storage = setup();

        // No context initially
        assert!(storage.load_context().unwrap().is_none());

        // Save context
        let mut ctx = Context::new();
        ctx.set_variable("key".to_string(), ContextValue::String("value".to_string()));
        storage.save_context(&ctx).unwrap();

        // Load context
        let loaded = storage.load_context().unwrap().unwrap();
        assert_eq!(
            loaded.get_variable("key"),
            Some(&ContextValue::String("value".to_string()))
        );

        // Delete context
        storage.delete_context().unwrap();
        assert!(storage.load_context().unwrap().is_none());
    }

    #[test]
    fn test_buffer_crud() {
        let mut storage = setup();

        // Add buffer
        let buffer = Buffer::from_named("test".to_string(), "Hello, world!".to_string());
        let id = storage.add_buffer(&buffer).unwrap();
        assert!(id > 0);

        // Get buffer
        let loaded = storage.get_buffer(id).unwrap().unwrap();
        assert_eq!(loaded.name, Some("test".to_string()));
        assert_eq!(loaded.content, "Hello, world!");

        // Get by name
        let by_name = storage.get_buffer_by_name("test").unwrap().unwrap();
        assert_eq!(by_name.id, Some(id));

        // List buffers
        let buffers = storage.list_buffers().unwrap();
        assert_eq!(buffers.len(), 1);

        // Update buffer
        let mut updated = loaded;
        updated.content = "Updated content".to_string();
        storage.update_buffer(&updated).unwrap();

        let reloaded = storage.get_buffer(id).unwrap().unwrap();
        assert_eq!(reloaded.content, "Updated content");

        // Delete buffer
        storage.delete_buffer(id).unwrap();
        assert!(storage.get_buffer(id).unwrap().is_none());
    }

    #[test]
    fn test_chunk_crud() {
        let mut storage = setup();

        // Create buffer first
        let buffer = Buffer::from_content("Hello, world!".to_string());
        let buffer_id = storage.add_buffer(&buffer).unwrap();

        // Add chunks
        let chunks = vec![
            Chunk::new(buffer_id, "Hello, ".to_string(), 0..7, 0),
            Chunk::new(buffer_id, "world!".to_string(), 7..13, 1),
        ];
        storage.add_chunks(buffer_id, &chunks).unwrap();

        // Get chunks
        let loaded = storage.get_chunks(buffer_id).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].content, "Hello, ");
        assert_eq!(loaded[1].content, "world!");

        // Chunk count
        assert_eq!(storage.chunk_count(buffer_id).unwrap(), 2);

        // Get single chunk
        let chunk_id = loaded[0].id.unwrap();
        let single = storage.get_chunk(chunk_id).unwrap().unwrap();
        assert_eq!(single.content, "Hello, ");

        // Delete chunks
        storage.delete_chunks(buffer_id).unwrap();
        assert_eq!(storage.chunk_count(buffer_id).unwrap(), 0);
    }

    #[test]
    fn test_cascade_delete() {
        let mut storage = setup();

        // Create buffer with chunks
        let buffer = Buffer::from_content("Hello, world!".to_string());
        let buffer_id = storage.add_buffer(&buffer).unwrap();

        let chunks = vec![Chunk::new(buffer_id, "Hello".to_string(), 0..5, 0)];
        storage.add_chunks(buffer_id, &chunks).unwrap();

        // Verify chunk exists
        assert_eq!(storage.chunk_count(buffer_id).unwrap(), 1);

        // Delete buffer - chunks should be deleted too
        storage.delete_buffer(buffer_id).unwrap();

        // Verify no orphan chunks (query all chunks)
        let count: i64 = storage
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_reset() {
        let mut storage = setup();

        // Add some data
        let ctx = Context::new();
        storage.save_context(&ctx).unwrap();

        let buffer = Buffer::from_content("test".to_string());
        storage.add_buffer(&buffer).unwrap();

        // Reset
        storage.reset().unwrap();

        // Verify empty
        assert!(storage.load_context().unwrap().is_none());
        assert_eq!(storage.buffer_count().unwrap(), 0);
    }

    #[test]
    fn test_stats() {
        let mut storage = setup();

        // Empty stats
        let stats = storage.stats().unwrap();
        assert_eq!(stats.buffer_count, 0);
        assert_eq!(stats.chunk_count, 0);
        assert!(!stats.has_context);

        // Add data
        let ctx = Context::new();
        storage.save_context(&ctx).unwrap();

        let buffer = Buffer::from_content("Hello, world!".to_string());
        let buffer_id = storage.add_buffer(&buffer).unwrap();

        let chunks = vec![Chunk::new(buffer_id, "Hello".to_string(), 0..5, 0)];
        storage.add_chunks(buffer_id, &chunks).unwrap();

        // Stats with data
        let stats = storage.stats().unwrap();
        assert_eq!(stats.buffer_count, 1);
        assert_eq!(stats.chunk_count, 1);
        assert!(stats.has_context);
        assert_eq!(stats.total_content_size, 13);
    }

    #[test]
    fn test_export_buffers() {
        let mut storage = setup();

        storage
            .add_buffer(&Buffer::from_content("First".to_string()))
            .unwrap();
        storage
            .add_buffer(&Buffer::from_content("Second".to_string()))
            .unwrap();

        let exported = storage.export_buffers().unwrap();
        assert_eq!(exported, "First\n\nSecond");
    }
}
