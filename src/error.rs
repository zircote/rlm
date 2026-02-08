//! Error types for RLM-RS operations.
//!
//! This module provides a comprehensive error hierarchy using `thiserror` for
//! all RLM operations including storage, chunking, I/O, and CLI commands.

use thiserror::Error;

/// Result type alias for RLM operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Comprehensive error types for RLM operations.
#[derive(Error, Debug)]
pub enum Error {
    /// Storage-related errors (database operations).
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    /// Chunking-related errors (text processing).
    #[error("chunking error: {0}")]
    Chunking(#[from] ChunkingError),

    /// I/O errors (file operations).
    #[error("I/O error: {0}")]
    Io(#[from] IoError),

    /// CLI command errors.
    #[error("command error: {0}")]
    Command(#[from] CommandError),

    /// Search errors.
    #[error("search error: {0}")]
    Search(#[from] SearchError),

    /// Invalid state errors.
    #[error("invalid state: {message}")]
    InvalidState {
        /// Description of the invalid state.
        message: String,
    },

    /// Configuration errors.
    #[error("configuration error: {message}")]
    Config {
        /// Description of the configuration error.
        message: String,
    },
}

/// Storage-specific errors for database operations.
#[derive(Error, Debug)]
pub enum StorageError {
    /// Database connection or query error.
    #[error("database error: {0}")]
    Database(String),

    /// Storage not initialized (init command not run).
    #[error("RLM not initialized. Run: rlm-cli init")]
    NotInitialized,

    /// Context not found in storage.
    #[error("context not found")]
    ContextNotFound,

    /// Buffer not found by ID or name.
    #[error("buffer not found: {identifier}")]
    BufferNotFound {
        /// Buffer ID or name that was not found.
        identifier: String,
    },

    /// Chunk not found by ID.
    #[error("chunk not found: {id}")]
    ChunkNotFound {
        /// Chunk ID that was not found.
        id: i64,
    },

    /// Schema migration error.
    #[error("migration error: {0}")]
    Migration(String),

    /// Transaction error.
    #[error("transaction error: {0}")]
    Transaction(String),

    /// Serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Vector search error (feature-gated).
    #[cfg(feature = "usearch-hnsw")]
    #[error("vector search error: {0}")]
    VectorSearch(String),

    /// Embedding error (feature-gated).
    #[cfg(feature = "fastembed-embeddings")]
    #[error("embedding error: {0}")]
    Embedding(String),
}

/// Chunking-specific errors for text processing.
#[derive(Error, Debug)]
pub enum ChunkingError {
    /// Invalid UTF-8 encountered at specific byte offset.
    #[error("invalid UTF-8 at byte offset {offset}")]
    InvalidUtf8 {
        /// Byte offset where invalid UTF-8 was found.
        offset: usize,
    },

    /// Chunk size exceeds maximum allowed.
    #[error("chunk size {size} exceeds maximum {max}")]
    ChunkTooLarge {
        /// Actual chunk size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// Invalid chunk configuration.
    #[error("invalid chunk configuration: {reason}")]
    InvalidConfig {
        /// Reason the configuration is invalid.
        reason: String,
    },

    /// Overlap exceeds chunk size.
    #[error("overlap {overlap} must be less than chunk size {size}")]
    OverlapTooLarge {
        /// Overlap size.
        overlap: usize,
        /// Chunk size.
        size: usize,
    },

    /// Parallel processing error.
    #[error("parallel processing failed: {reason}")]
    ParallelFailed {
        /// Reason for failure.
        reason: String,
    },

    /// Semantic analysis error.
    #[error("semantic analysis failed: {0}")]
    SemanticFailed(String),

    /// Regex compilation error.
    #[error("regex error: {0}")]
    Regex(String),

    /// Unknown chunking strategy.
    #[error("unknown chunking strategy: {name}")]
    UnknownStrategy {
        /// Name of the unknown strategy.
        name: String,
    },
}

/// I/O-specific errors for file operations.
#[derive(Error, Debug)]
pub enum IoError {
    /// File not found.
    #[error("file not found: {path}")]
    FileNotFound {
        /// Path to the file that was not found.
        path: String,
    },

    /// Failed to read file.
    #[error("failed to read file: {path}: {reason}")]
    ReadFailed {
        /// Path to the file.
        path: String,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to write file.
    #[error("failed to write file: {path}: {reason}")]
    WriteFailed {
        /// Path to the file.
        path: String,
        /// Reason for failure.
        reason: String,
    },

    /// Memory mapping error.
    #[error("memory mapping failed: {path}: {reason}")]
    MmapFailed {
        /// Path to the file.
        path: String,
        /// Reason for failure.
        reason: String,
    },

    /// Directory creation error.
    #[error("failed to create directory: {path}: {reason}")]
    DirectoryFailed {
        /// Path to the directory.
        path: String,
        /// Reason for failure.
        reason: String,
    },

    /// Path traversal security error.
    #[error("path traversal denied: {path}")]
    PathTraversal {
        /// Path that was denied.
        path: String,
    },

    /// Generic I/O error wrapper.
    #[error("I/O error: {0}")]
    Generic(String),
}

/// Search-specific errors for vector search operations.
#[derive(Error, Debug)]
pub enum SearchError {
    /// HNSW index operation failed.
    #[error("index error: {message}")]
    IndexError {
        /// Error message.
        message: String,
    },

    /// Vector dimension mismatch.
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch {
        /// Expected dimensions.
        expected: usize,
        /// Actual dimensions.
        got: usize,
    },

    /// Feature not enabled.
    #[error("feature not enabled: {feature}")]
    FeatureNotEnabled {
        /// Name of the required feature.
        feature: String,
    },

    /// Query error.
    #[error("query error: {message}")]
    QueryError {
        /// Error message.
        message: String,
    },
}

/// CLI command-specific errors.
#[derive(Error, Debug)]
pub enum CommandError {
    /// Unknown command.
    #[error("unknown command: {0}")]
    UnknownCommand(String),

    /// Invalid argument provided.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// Missing required argument.
    #[error("missing required argument: {0}")]
    MissingArgument(String),

    /// Command execution failed.
    #[error("command execution failed: {0}")]
    ExecutionFailed(String),

    /// User cancelled operation.
    #[error("operation cancelled by user")]
    Cancelled,

    /// Output format error.
    #[error("output format error: {0}")]
    OutputFormat(String),
}

// Implement From traits for standard library errors

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(IoError::Generic(err.to_string()))
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::Storage(StorageError::Database(err.to_string()))
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Database(err.to_string())
    }
}

impl From<regex::Error> for ChunkingError {
    fn from(err: regex::Error) -> Self {
        Self::Regex(err.to_string())
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for ChunkingError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::InvalidUtf8 {
            offset: err.utf8_error().valid_up_to(),
        }
    }
}

impl From<std::str::Utf8Error> for ChunkingError {
    fn from(err: std::str::Utf8Error) -> Self {
        Self::InvalidUtf8 {
            offset: err.valid_up_to(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::InvalidState {
            message: "test error".to_string(),
        };
        assert_eq!(err.to_string(), "invalid state: test error");
    }

    #[test]
    fn test_storage_error_display() {
        let err = StorageError::NotInitialized;
        assert_eq!(err.to_string(), "RLM not initialized. Run: rlm-cli init");

        let err = StorageError::BufferNotFound {
            identifier: "test-buffer".to_string(),
        };
        assert_eq!(err.to_string(), "buffer not found: test-buffer");
    }

    #[test]
    fn test_chunking_error_display() {
        let err = ChunkingError::InvalidUtf8 { offset: 42 };
        assert_eq!(err.to_string(), "invalid UTF-8 at byte offset 42");

        let err = ChunkingError::OverlapTooLarge {
            overlap: 100,
            size: 50,
        };
        assert_eq!(
            err.to_string(),
            "overlap 100 must be less than chunk size 50"
        );
    }

    #[test]
    fn test_io_error_display() {
        let err = IoError::FileNotFound {
            path: "/tmp/test.txt".to_string(),
        };
        assert_eq!(err.to_string(), "file not found: /tmp/test.txt");
    }

    #[test]
    fn test_command_error_display() {
        let err = CommandError::MissingArgument("--file".to_string());
        assert_eq!(err.to_string(), "missing required argument: --file");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn test_error_from_storage() {
        let storage_err = StorageError::NotInitialized;
        let err: Error = storage_err.into();
        assert!(matches!(err, Error::Storage(_)));
    }

    #[test]
    fn test_error_from_chunking() {
        let chunk_err = ChunkingError::InvalidUtf8 { offset: 0 };
        let err: Error = chunk_err.into();
        assert!(matches!(err, Error::Chunking(_)));
    }

    #[test]
    fn test_error_from_command() {
        let cmd_err = CommandError::Cancelled;
        let err: Error = cmd_err.into();
        assert!(matches!(err, Error::Command(_)));
    }

    #[test]
    fn test_error_config() {
        let err = Error::Config {
            message: "bad config".to_string(),
        };
        assert_eq!(err.to_string(), "configuration error: bad config");
    }

    #[test]
    fn test_storage_error_variants() {
        let err = StorageError::Database("connection failed".to_string());
        assert!(err.to_string().contains("connection failed"));

        let err = StorageError::ContextNotFound;
        assert_eq!(err.to_string(), "context not found");

        let err = StorageError::ChunkNotFound { id: 42 };
        assert_eq!(err.to_string(), "chunk not found: 42");

        let err = StorageError::Migration("schema error".to_string());
        assert!(err.to_string().contains("schema error"));

        let err = StorageError::Transaction("rollback".to_string());
        assert!(err.to_string().contains("rollback"));

        let err = StorageError::Serialization("invalid json".to_string());
        assert!(err.to_string().contains("invalid json"));
    }

    #[test]
    fn test_chunking_error_variants() {
        let err = ChunkingError::ChunkTooLarge {
            size: 1000,
            max: 500,
        };
        assert!(err.to_string().contains("1000"));
        assert!(err.to_string().contains("500"));

        let err = ChunkingError::InvalidConfig {
            reason: "bad overlap".to_string(),
        };
        assert!(err.to_string().contains("bad overlap"));

        let err = ChunkingError::ParallelFailed {
            reason: "thread panic".to_string(),
        };
        assert!(err.to_string().contains("thread panic"));

        let err = ChunkingError::SemanticFailed("model error".to_string());
        assert!(err.to_string().contains("model error"));

        let err = ChunkingError::Regex("invalid pattern".to_string());
        assert!(err.to_string().contains("invalid pattern"));

        let err = ChunkingError::UnknownStrategy {
            name: "foobar".to_string(),
        };
        assert!(err.to_string().contains("foobar"));
    }

    #[test]
    fn test_io_error_variants() {
        let err = IoError::ReadFailed {
            path: "/tmp/test".to_string(),
            reason: "permission denied".to_string(),
        };
        assert!(err.to_string().contains("/tmp/test"));
        assert!(err.to_string().contains("permission denied"));

        let err = IoError::WriteFailed {
            path: "/tmp/out".to_string(),
            reason: "disk full".to_string(),
        };
        assert!(err.to_string().contains("disk full"));

        let err = IoError::MmapFailed {
            path: "/tmp/big".to_string(),
            reason: "out of memory".to_string(),
        };
        assert!(err.to_string().contains("memory mapping"));

        let err = IoError::DirectoryFailed {
            path: "/tmp/dir".to_string(),
            reason: "exists".to_string(),
        };
        assert!(err.to_string().contains("directory"));

        let err = IoError::PathTraversal {
            path: "../etc/passwd".to_string(),
        };
        assert!(err.to_string().contains("traversal"));

        let err = IoError::Generic("unknown error".to_string());
        assert!(err.to_string().contains("unknown error"));
    }

    #[test]
    fn test_command_error_variants() {
        let err = CommandError::UnknownCommand("foo".to_string());
        assert!(err.to_string().contains("unknown command"));

        let err = CommandError::InvalidArgument("--bad".to_string());
        assert!(err.to_string().contains("invalid argument"));

        let err = CommandError::ExecutionFailed("timeout".to_string());
        assert!(err.to_string().contains("execution failed"));

        let err = CommandError::Cancelled;
        assert!(err.to_string().contains("cancelled"));

        let err = CommandError::OutputFormat("json error".to_string());
        assert!(err.to_string().contains("output format"));
    }

    #[test]
    fn test_from_rusqlite_error_to_error() {
        let rusqlite_err = rusqlite::Error::InvalidQuery;
        let err: Error = rusqlite_err.into();
        assert!(matches!(err, Error::Storage(StorageError::Database(_))));
    }

    #[test]
    fn test_from_rusqlite_error_to_storage_error() {
        let rusqlite_err = rusqlite::Error::InvalidQuery;
        let err: StorageError = rusqlite_err.into();
        assert!(matches!(err, StorageError::Database(_)));
    }

    #[test]
    #[allow(clippy::invalid_regex)]
    fn test_from_regex_error_to_chunking_error() {
        let regex_err = regex::Regex::new("[invalid").unwrap_err();
        let err: ChunkingError = regex_err.into();
        assert!(matches!(err, ChunkingError::Regex(_)));
    }

    #[test]
    fn test_from_serde_json_error_to_storage_error() {
        let json_err: serde_json::Error = serde_json::from_str::<i32>("invalid").unwrap_err();
        let err: StorageError = json_err.into();
        assert!(matches!(err, StorageError::Serialization(_)));
    }

    #[test]
    fn test_from_string_utf8_error_to_chunking_error() {
        // Create invalid UTF-8 bytes
        let invalid_bytes = vec![0xff, 0xfe];
        let utf8_err = String::from_utf8(invalid_bytes).unwrap_err();
        let err: ChunkingError = utf8_err.into();
        assert!(matches!(err, ChunkingError::InvalidUtf8 { .. }));
    }

    #[test]
    fn test_from_str_utf8_error_to_chunking_error() {
        // Create invalid UTF-8 bytes at runtime to avoid lint warning
        let invalid_bytes: Vec<u8> = vec![0xff, 0xfe];
        let utf8_err = std::str::from_utf8(&invalid_bytes).unwrap_err();
        let err: ChunkingError = utf8_err.into();
        assert!(matches!(err, ChunkingError::InvalidUtf8 { .. }));
    }
}
