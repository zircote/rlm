//! Inode allocation and path resolution for the FUSE filesystem.
//!
//! This module provides deterministic inode allocation using fixed ranges
//! for different entity types (buffers, chunks, embeddings) and functions
//! for bidirectional conversion between inodes and entity IDs.

// These casts are intentional for inode ↔ entity ID conversion.
// IDs are always non-negative in practice, and the ranges are designed
// to fit within both i64 and u64 positive ranges.
// File extension checks are case-sensitive by design since we control the virtual filesystem filenames.
#![allow(
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::case_sensitive_file_extension_comparisons
)]

/// Root directory inode (standard FUSE convention).
pub const INODE_ROOT: u64 = 1;

/// `/buffers/` directory inode.
pub const INODE_BUFFERS_DIR: u64 = 2;

/// `/chunks/` directory inode.
pub const INODE_CHUNKS_DIR: u64 = 3;

/// `/embeddings/` directory inode.
pub const INODE_EMBEDDINGS_DIR: u64 = 4;

/// `/search/` directory inode.
pub const INODE_SEARCH_DIR: u64 = 5;

/// `/stats.json` file inode.
pub const INODE_STATS_FILE: u64 = 6;

/// `/search/query.txt` file inode.
pub const INODE_SEARCH_QUERY: u64 = 7;

/// `/search/results.json` file inode.
pub const INODE_SEARCH_RESULTS: u64 = 8;

// Dynamic inode ranges - chosen to avoid overlap
/// Base inode for buffer files (`/buffers/{id}.txt`).
/// Buffer ID N maps to inode `INODE_BUFFER_BASE + N`.
pub const INODE_BUFFER_BASE: u64 = 1_000_000;

/// Base inode for chunk buffer directories (`/chunks/{buffer_id}/`).
/// Buffer ID N maps to inode `INODE_CHUNK_BUFFER_DIR_BASE + N`.
pub const INODE_CHUNK_BUFFER_DIR_BASE: u64 = 10_000_000;

/// Base inode for chunk files (`/chunks/{buffer_id}/{index}.txt`).
/// Chunk ID N maps to inode `INODE_CHUNK_BASE + N`.
pub const INODE_CHUNK_BASE: u64 = 100_000_000;

/// Base inode for chunk metadata files (`/chunks/{buffer_id}/metadata.json`).
/// Buffer ID N maps to inode `INODE_CHUNK_METADATA_BASE + N`.
pub const INODE_CHUNK_METADATA_BASE: u64 = 150_000_000;

/// Base inode for embedding files (`/embeddings/{chunk_id}.json`).
/// Chunk ID N maps to inode `INODE_EMBEDDING_BASE + N`.
pub const INODE_EMBEDDING_BASE: u64 = 200_000_000;

/// Classification of inode types for dispatch in FUSE operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InodeType {
    /// Root directory (`/`).
    Root,
    /// Buffers directory (`/buffers/`).
    BuffersDir,
    /// Chunks directory (`/chunks/`).
    ChunksDir,
    /// Embeddings directory (`/embeddings/`).
    EmbeddingsDir,
    /// Search directory (`/search/`).
    SearchDir,
    /// Stats file (`/stats.json`).
    StatsFile,
    /// Search query file (`/search/query.txt`).
    SearchQuery,
    /// Search results file (`/search/results.json`).
    SearchResults,
    /// Buffer content file (`/buffers/{id}.txt`) with buffer ID.
    BufferFile(i64),
    /// Chunk buffer directory (`/chunks/{buffer_id}/`) with buffer ID.
    ChunkBufferDir(i64),
    /// Chunk content file (`/chunks/{buffer_id}/{index}.txt`) with chunk ID.
    ChunkFile(i64),
    /// Chunk metadata file (`/chunks/{buffer_id}/metadata.json`) with buffer ID.
    ChunkMetadata(i64),
    /// Embedding file (`/embeddings/{chunk_id}.json`) with chunk ID.
    EmbeddingFile(i64),
    /// Unknown/invalid inode.
    Unknown,
}

/// Converts a buffer ID to its corresponding inode.
#[must_use]
pub const fn buffer_id_to_inode(buffer_id: i64) -> u64 {
    INODE_BUFFER_BASE + buffer_id as u64
}

/// Converts an inode to a buffer ID if it's in the buffer range.
#[must_use]
pub const fn inode_to_buffer_id(inode: u64) -> Option<i64> {
    if inode >= INODE_BUFFER_BASE && inode < INODE_CHUNK_BUFFER_DIR_BASE {
        Some((inode - INODE_BUFFER_BASE) as i64)
    } else {
        None
    }
}

/// Converts a buffer ID to its chunk directory inode.
#[must_use]
pub const fn chunk_buffer_dir_to_inode(buffer_id: i64) -> u64 {
    INODE_CHUNK_BUFFER_DIR_BASE + buffer_id as u64
}

/// Converts an inode to a buffer ID if it's a chunk buffer directory.
#[must_use]
pub const fn inode_to_chunk_buffer_dir_id(inode: u64) -> Option<i64> {
    if inode >= INODE_CHUNK_BUFFER_DIR_BASE && inode < INODE_CHUNK_BASE {
        Some((inode - INODE_CHUNK_BUFFER_DIR_BASE) as i64)
    } else {
        None
    }
}

/// Converts a chunk ID to its corresponding inode.
#[must_use]
pub const fn chunk_id_to_inode(chunk_id: i64) -> u64 {
    INODE_CHUNK_BASE + chunk_id as u64
}

/// Converts an inode to a chunk ID if it's in the chunk range.
#[must_use]
pub const fn inode_to_chunk_id(inode: u64) -> Option<i64> {
    if inode >= INODE_CHUNK_BASE && inode < INODE_CHUNK_METADATA_BASE {
        Some((inode - INODE_CHUNK_BASE) as i64)
    } else {
        None
    }
}

/// Converts a buffer ID to its chunk metadata file inode.
#[must_use]
pub const fn chunk_metadata_to_inode(buffer_id: i64) -> u64 {
    INODE_CHUNK_METADATA_BASE + buffer_id as u64
}

/// Converts an inode to a buffer ID if it's a chunk metadata file.
#[must_use]
pub const fn inode_to_chunk_metadata_buffer_id(inode: u64) -> Option<i64> {
    if inode >= INODE_CHUNK_METADATA_BASE && inode < INODE_EMBEDDING_BASE {
        Some((inode - INODE_CHUNK_METADATA_BASE) as i64)
    } else {
        None
    }
}

/// Converts a chunk ID to its embedding file inode.
#[must_use]
pub const fn embedding_chunk_id_to_inode(chunk_id: i64) -> u64 {
    INODE_EMBEDDING_BASE + chunk_id as u64
}

/// Converts an inode to a chunk ID if it's an embedding file.
#[must_use]
pub const fn inode_to_embedding_chunk_id(inode: u64) -> Option<i64> {
    if inode >= INODE_EMBEDDING_BASE {
        Some((inode - INODE_EMBEDDING_BASE) as i64)
    } else {
        None
    }
}

/// Classifies an inode into its corresponding type.
#[must_use]
pub const fn classify_inode(inode: u64) -> InodeType {
    match inode {
        INODE_ROOT => InodeType::Root,
        INODE_BUFFERS_DIR => InodeType::BuffersDir,
        INODE_CHUNKS_DIR => InodeType::ChunksDir,
        INODE_EMBEDDINGS_DIR => InodeType::EmbeddingsDir,
        INODE_SEARCH_DIR => InodeType::SearchDir,
        INODE_STATS_FILE => InodeType::StatsFile,
        INODE_SEARCH_QUERY => InodeType::SearchQuery,
        INODE_SEARCH_RESULTS => InodeType::SearchResults,
        _ => {
            // Check dynamic ranges in order
            if let Some(id) = inode_to_buffer_id(inode) {
                InodeType::BufferFile(id)
            } else if let Some(id) = inode_to_chunk_buffer_dir_id(inode) {
                InodeType::ChunkBufferDir(id)
            } else if let Some(id) = inode_to_chunk_id(inode) {
                InodeType::ChunkFile(id)
            } else if let Some(id) = inode_to_chunk_metadata_buffer_id(inode) {
                InodeType::ChunkMetadata(id)
            } else if let Some(id) = inode_to_embedding_chunk_id(inode) {
                InodeType::EmbeddingFile(id)
            } else {
                InodeType::Unknown
            }
        }
    }
}

/// Parses a buffer filename like "1.txt" or "42.md" into a buffer ID.
#[must_use]
pub fn parse_buffer_filename(name: &str) -> Option<i64> {
    // Split on '.' and parse the first part as an ID
    let stem = name.split('.').next()?;
    stem.parse().ok()
}

/// Parses a chunk filename like "0.txt" or "99.txt" into a chunk index.
#[must_use]
pub fn parse_chunk_filename(name: &str) -> Option<usize> {
    if !name.ends_with(".txt") {
        return None;
    }
    let stem = name.strip_suffix(".txt")?;
    stem.parse().ok()
}

/// Parses an embedding filename like "1.json" into a chunk ID.
#[must_use]
pub fn parse_embedding_filename(name: &str) -> Option<i64> {
    if !name.ends_with(".json") {
        return None;
    }
    let stem = name.strip_suffix(".json")?;
    stem.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_root_inode_is_one() {
        assert_eq!(INODE_ROOT, 1);
    }

    #[test]
    fn test_static_inodes_are_unique() {
        let statics = [
            INODE_ROOT,
            INODE_BUFFERS_DIR,
            INODE_CHUNKS_DIR,
            INODE_EMBEDDINGS_DIR,
            INODE_SEARCH_DIR,
            INODE_STATS_FILE,
            INODE_SEARCH_QUERY,
            INODE_SEARCH_RESULTS,
        ];
        let unique: HashSet<_> = statics.iter().collect();
        assert_eq!(statics.len(), unique.len());
    }

    #[test]
    fn test_buffer_id_to_inode() {
        assert_eq!(buffer_id_to_inode(1), INODE_BUFFER_BASE + 1);
        assert_eq!(buffer_id_to_inode(100), INODE_BUFFER_BASE + 100);
    }

    #[test]
    fn test_inode_to_buffer_id() {
        assert_eq!(inode_to_buffer_id(INODE_BUFFER_BASE + 1), Some(1));
        assert_eq!(inode_to_buffer_id(INODE_BUFFER_BASE + 100), Some(100));
        assert_eq!(inode_to_buffer_id(INODE_ROOT), None);
    }

    #[test]
    fn test_buffer_inode_roundtrip() {
        for id in [1, 10, 100, 999_999] {
            let inode = buffer_id_to_inode(id);
            assert_eq!(inode_to_buffer_id(inode), Some(id));
        }
    }

    #[test]
    fn test_chunk_id_to_inode() {
        assert_eq!(chunk_id_to_inode(1), INODE_CHUNK_BASE + 1);
    }

    #[test]
    fn test_inode_to_chunk_id() {
        assert_eq!(inode_to_chunk_id(INODE_CHUNK_BASE + 42), Some(42));
        assert_eq!(inode_to_chunk_id(INODE_ROOT), None);
    }

    #[test]
    fn test_chunk_inode_roundtrip() {
        for id in [1, 10, 100, 999_999] {
            let inode = chunk_id_to_inode(id);
            assert_eq!(inode_to_chunk_id(inode), Some(id));
        }
    }

    #[test]
    fn test_embedding_chunk_id_to_inode() {
        assert_eq!(embedding_chunk_id_to_inode(5), INODE_EMBEDDING_BASE + 5);
    }

    #[test]
    fn test_inode_to_embedding_chunk_id() {
        assert_eq!(
            inode_to_embedding_chunk_id(INODE_EMBEDDING_BASE + 5),
            Some(5)
        );
        assert_eq!(inode_to_embedding_chunk_id(INODE_ROOT), None);
    }

    #[test]
    fn test_classify_root() {
        assert!(matches!(classify_inode(INODE_ROOT), InodeType::Root));
    }

    #[test]
    fn test_classify_buffers_dir() {
        assert!(matches!(
            classify_inode(INODE_BUFFERS_DIR),
            InodeType::BuffersDir
        ));
    }

    #[test]
    fn test_classify_buffer_file() {
        let inode = buffer_id_to_inode(42);
        assert!(matches!(classify_inode(inode), InodeType::BufferFile(42)));
    }

    #[test]
    fn test_classify_chunk_file() {
        let inode = chunk_id_to_inode(123);
        assert!(matches!(classify_inode(inode), InodeType::ChunkFile(123)));
    }

    #[test]
    fn test_classify_chunk_buffer_dir() {
        let inode = chunk_buffer_dir_to_inode(7);
        assert!(matches!(
            classify_inode(inode),
            InodeType::ChunkBufferDir(7)
        ));
    }

    #[test]
    fn test_classify_embedding_file() {
        let inode = embedding_chunk_id_to_inode(99);
        assert!(matches!(
            classify_inode(inode),
            InodeType::EmbeddingFile(99)
        ));
    }

    #[test]
    fn test_parse_buffer_filename() {
        assert_eq!(parse_buffer_filename("1.txt"), Some(1));
        assert_eq!(parse_buffer_filename("42.md"), Some(42));
        assert_eq!(parse_buffer_filename("invalid"), None);
        assert_eq!(parse_buffer_filename(""), None);
    }

    #[test]
    fn test_parse_chunk_filename() {
        assert_eq!(parse_chunk_filename("0.txt"), Some(0));
        assert_eq!(parse_chunk_filename("99.txt"), Some(99));
        assert_eq!(parse_chunk_filename("metadata.json"), None);
        assert_eq!(parse_chunk_filename("0.md"), None);
    }

    #[test]
    fn test_parse_embedding_filename() {
        assert_eq!(parse_embedding_filename("1.json"), Some(1));
        assert_eq!(parse_embedding_filename("42.json"), Some(42));
        assert_eq!(parse_embedding_filename("1.txt"), None);
    }

    #[test]
    fn test_inode_ranges_dont_overlap() {
        // Ensure buffer range doesn't overlap with chunk buffer dir range
        // Buffer IDs can go up to ~8 million before overlapping
        let max_buffer_inode = INODE_BUFFER_BASE + 8_000_000;
        assert!(max_buffer_inode < INODE_CHUNK_BUFFER_DIR_BASE);

        // Ensure chunk buffer dir range doesn't overlap with chunk range
        // Chunk buffer dir can have ~89 million buffer IDs
        let max_chunk_buffer_dir_inode = INODE_CHUNK_BUFFER_DIR_BASE + 89_000_000;
        assert!(max_chunk_buffer_dir_inode < INODE_CHUNK_BASE);

        // Ensure chunk range doesn't overlap with chunk metadata range
        // Chunks can have ~49 million IDs
        let max_chunk_inode = INODE_CHUNK_BASE + 49_000_000;
        assert!(max_chunk_inode < INODE_CHUNK_METADATA_BASE);

        // Ensure chunk metadata range doesn't overlap with embedding range
        // Chunk metadata can have ~49 million buffer IDs
        let max_chunk_metadata_inode = INODE_CHUNK_METADATA_BASE + 49_000_000;
        assert!(max_chunk_metadata_inode < INODE_EMBEDDING_BASE);
    }
}
