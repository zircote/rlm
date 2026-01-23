//! # RLM-RS
//!
//! Recursive Language Model REPL for Claude Code.
//!
//! RLM-RS is a CLI tool for handling large context files via chunking and
//! recursive sub-LLM calls. It allows LLMs to process prompts far exceeding
//! their context windows by decomposing content into manageable chunks.
//!
//! ## Features
//!
//! - **Chunking**: Multiple strategies (fixed, semantic, parallel) for splitting content
//! - **`SQLite` Storage**: Persistent state with transaction support
//! - **Memory Mapping**: Efficient handling of large files
//! - **Unicode Aware**: Proper grapheme cluster handling

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(missing_docs)]
// Note: unsafe is needed for memory-mapped I/O (memmap2)
#![warn(unsafe_code)]

pub mod chunking;
pub mod cli;
pub mod core;
pub mod embedding;
pub mod error;
#[cfg(feature = "fuse")]
pub mod fuse;
pub mod io;
pub mod search;
pub mod storage;

// Re-export commonly used types at crate root
pub use error::{Error, Result};

// Re-export core domain types
pub use core::{Buffer, BufferMetadata, Chunk, ChunkMetadata, Context, ContextValue};

// Re-export storage types
pub use storage::{DEFAULT_DB_PATH, SqliteStorage, Storage};

// Re-export chunking types
pub use chunking::{Chunker, FixedChunker, SemanticChunker, available_strategies, create_chunker};

// Re-export CLI types
pub use cli::{Cli, Commands, OutputFormat};

// Re-export embedding types
#[cfg(feature = "fastembed-embeddings")]
pub use embedding::FastEmbedEmbedder;
pub use embedding::{
    DEFAULT_DIMENSIONS, Embedder, FallbackEmbedder, cosine_similarity, create_embedder,
};

// Re-export search types
pub use search::{
    DEFAULT_SIMILARITY_THRESHOLD, DEFAULT_TOP_K, RrfConfig, SearchConfig, SearchResult,
    buffer_fully_embedded, embed_buffer_chunks, hybrid_search, reciprocal_rank_fusion, search_bm25,
    search_semantic, weighted_rrf,
};

// Re-export FUSE types (feature-gated)
#[cfg(feature = "fuse")]
pub use fuse::{RlmFs, mount};
