//! Storage layer for RLM-RS.
//!
//! Provides persistent storage for RLM state using `SQLite`. The storage
//! layer handles contexts, buffers, chunks, and metadata with proper
//! transaction support.

pub mod schema;
pub mod sqlite;
pub mod traits;

pub use schema::{CURRENT_SCHEMA_VERSION, SCHEMA_SQL};
pub use sqlite::SqliteStorage;
pub use traits::{Storage, StorageStats};

/// Default database file name.
pub const DEFAULT_DB_NAME: &str = "rlm-state.db";

/// Default database path relative to project root.
pub const DEFAULT_DB_PATH: &str = ".rlm/rlm-state.db";
