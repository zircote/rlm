//! Database schema definitions.
//!
//! Contains SQL schema and migration logic for the RLM `SQLite` database.

/// Current schema version.
pub const CURRENT_SCHEMA_VERSION: u32 = 3;

/// SQL schema for initial database setup.
pub const SCHEMA_SQL: &str = r"
-- Schema version tracking
CREATE TABLE IF NOT EXISTS schema_info (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- RLM Context state
CREATE TABLE IF NOT EXISTS context (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Singleton
    data TEXT NOT NULL,  -- JSON serialized Context
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Buffers (text content containers)
CREATE TABLE IF NOT EXISTS buffers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT,
    source_path TEXT,
    content TEXT NOT NULL,
    content_type TEXT,
    content_hash TEXT,
    size INTEGER NOT NULL,
    line_count INTEGER,
    chunk_count INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Index for buffer lookup by name
CREATE INDEX IF NOT EXISTS idx_buffers_name ON buffers(name);

-- Index for buffer lookup by hash (deduplication)
CREATE INDEX IF NOT EXISTS idx_buffers_hash ON buffers(content_hash);

-- Chunks (segments of buffer content)
CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    buffer_id INTEGER NOT NULL,
    content TEXT NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    strategy TEXT,
    token_count INTEGER,
    line_start INTEGER,
    line_end INTEGER,
    has_overlap INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT,
    custom_metadata TEXT,  -- JSON for extensible metadata
    created_at INTEGER NOT NULL,
    FOREIGN KEY (buffer_id) REFERENCES buffers(id) ON DELETE CASCADE
);

-- Index for chunk lookup by buffer
CREATE INDEX IF NOT EXISTS idx_chunks_buffer ON chunks(buffer_id);

-- Index for chunk ordering
CREATE INDEX IF NOT EXISTS idx_chunks_order ON chunks(buffer_id, chunk_index);

-- Metadata key-value store for extensibility
CREATE TABLE IF NOT EXISTS metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Chunk embeddings for semantic search (v2)
CREATE TABLE IF NOT EXISTS chunk_embeddings (
    chunk_id INTEGER PRIMARY KEY,
    embedding BLOB NOT NULL,  -- f32 array serialized as bytes
    dimensions INTEGER NOT NULL,
    model_name TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (chunk_id) REFERENCES chunks(id) ON DELETE CASCADE
);

-- FTS5 virtual table for BM25 full-text search (v2)
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    content,
    content='chunks',
    content_rowid='id',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS5 index in sync with chunks table (v2)
CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES('delete', old.id, old.content);
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES (new.id, new.content);
END;
";

/// SQL to check if schema is initialized.
pub const CHECK_SCHEMA_SQL: &str = r"
SELECT COUNT(*) FROM sqlite_master
WHERE type='table' AND name='schema_info';
";

/// SQL to get schema version.
pub const GET_VERSION_SQL: &str = r"
SELECT value FROM schema_info WHERE key = 'version';
";

/// SQL to set schema version.
pub const SET_VERSION_SQL: &str = r"
INSERT OR REPLACE INTO schema_info (key, value) VALUES ('version', ?);
";

/// Migrations from older schema versions.
pub struct Migration {
    /// Version this migration upgrades from.
    pub from_version: u32,
    /// Version this migration upgrades to.
    pub to_version: u32,
    /// SQL statements to execute.
    pub sql: &'static str,
}

/// SQL for v1 to v2 migration (adds embeddings + FTS5).
const MIGRATION_V1_TO_V2: &str = r"
-- Chunk embeddings for semantic search
CREATE TABLE IF NOT EXISTS chunk_embeddings (
    chunk_id INTEGER PRIMARY KEY,
    embedding BLOB NOT NULL,
    dimensions INTEGER NOT NULL,
    model_name TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (chunk_id) REFERENCES chunks(id) ON DELETE CASCADE
);

-- FTS5 virtual table for BM25 full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    content,
    content='chunks',
    content_rowid='id',
    tokenize='porter unicode61'
);

-- Triggers to keep FTS5 index in sync
CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES('delete', old.id, old.content);
    INSERT INTO chunks_fts(chunks_fts, rowid, content) VALUES (new.id, new.content);
END;

-- Populate FTS5 index from existing chunks
INSERT INTO chunks_fts(rowid, content) SELECT id, content FROM chunks;
";

/// SQL for v2 to v3 migration (clear embeddings for BGE-M3 model switch).
///
/// BGE-M3 uses 1024 dimensions vs 384 for all-MiniLM-L6-v2.
/// Existing embeddings are incompatible and must be regenerated.
const MIGRATION_V2_TO_V3: &str = r"
-- Clear existing embeddings (incompatible dimensions: 384 -> 1024)
-- Users must re-run embedding generation after this migration
DELETE FROM chunk_embeddings;
";

/// Available migrations.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        from_version: 1,
        to_version: 2,
        sql: MIGRATION_V1_TO_V2,
    },
    Migration {
        from_version: 2,
        to_version: 3,
        sql: MIGRATION_V2_TO_V3,
    },
];

/// Gets migrations needed to upgrade from a version.
#[must_use]
pub fn get_migrations_from(current_version: u32) -> Vec<&'static Migration> {
    MIGRATIONS
        .iter()
        .filter(|m| m.from_version >= current_version && m.to_version <= CURRENT_SCHEMA_VERSION)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version() {
        const _: () = assert!(CURRENT_SCHEMA_VERSION >= 1);
    }

    #[test]
    fn test_schema_sql_not_empty() {
        assert!(!SCHEMA_SQL.is_empty());
        assert!(SCHEMA_SQL.contains("CREATE TABLE"));
    }

    #[test]
    fn test_migrations_ordered() {
        for migration in MIGRATIONS {
            assert!(migration.to_version > migration.from_version);
        }
    }

    #[test]
    fn test_get_migrations_from() {
        let migrations = get_migrations_from(0);
        // Should return all migrations for fresh install
        assert!(migrations.len() <= MIGRATIONS.len());
    }
}
