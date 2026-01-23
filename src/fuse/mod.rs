// Allow unsafe for libc::getuid()/getgid() calls required by FUSE file attributes
#![allow(unsafe_code)]

//! FUSE virtual filesystem interface for rlm-rs.
//!
//! This module provides a read-only FUSE filesystem that exposes rlm-rs data
//! (buffers, chunks, embeddings, search, stats) as files and directories
//! accessible via standard POSIX tools (ls, cat, grep, tail, etc.).
//!
//! # Virtual Filesystem Structure
//!
//! ```text
//! /mnt/rlm/                    (mountpoint)
//! ├── buffers/                 (directory - all buffers)
//! │   ├── 1.txt                (buffer content)
//! │   ├── 2.md
//! │   └── ...
//! ├── chunks/                  (directory - chunks by buffer)
//! │   ├── 1/                   (buffer 1's chunks)
//! │   │   ├── 0.txt            (chunk content)
//! │   │   ├── 1.txt
//! │   │   └── metadata.json    (chunk metadata)
//! │   └── 2/
//! │       └── ...
//! ├── embeddings/              (directory - embedding vectors)
//! │   ├── 1.json               (embedding for chunk 1)
//! │   └── ...
//! ├── search/                  (directory - search interface)
//! │   ├── query.txt            (write query here → triggers search)
//! │   └── results.json         (read search results)
//! └── stats.json               (storage statistics)
//! ```
//!
//! # Example Usage
//!
//! ```bash
//! # Mount the filesystem
//! rlm-rs mount /mnt/rlm
//!
//! # List all buffers
//! ls /mnt/rlm/buffers/
//!
//! # Read buffer content
//! cat /mnt/rlm/buffers/1.txt
//!
//! # Semantic search
//! echo "error handling in rust" > /mnt/rlm/search/query.txt
//! cat /mnt/rlm/search/results.json | jq '.[:5]'
//!
//! # Unmount
//! fusermount -u /mnt/rlm
//! ```

pub mod inodes;

use crate::embedding::{Embedder, create_embedder};
use crate::error::Result;
use crate::search::{SearchConfig, hybrid_search};
use crate::storage::{SqliteStorage, Storage};

use fuser::{
    FileAttr, FileType, Filesystem, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyWrite, Request,
};
use inodes::{
    INODE_BUFFERS_DIR, INODE_CHUNKS_DIR, INODE_EMBEDDINGS_DIR, INODE_ROOT, INODE_SEARCH_DIR,
    INODE_SEARCH_QUERY, INODE_SEARCH_RESULTS, INODE_STATS_FILE, InodeType, buffer_id_to_inode,
    chunk_buffer_dir_to_inode, chunk_id_to_inode, chunk_metadata_to_inode, classify_inode,
    embedding_chunk_id_to_inode, parse_buffer_filename, parse_chunk_filename,
    parse_embedding_filename,
};
use libc::{EINVAL, EIO, ENOENT, EPERM};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

/// Default time-to-live for cached attributes.
const TTL: Duration = Duration::from_secs(1);

/// Read-only FUSE filesystem exposing rlm-rs data.
pub struct RlmFs {
    /// Storage backend (thread-safe).
    storage: Arc<RwLock<SqliteStorage>>,
    /// Embedder for semantic search (thread-safe).
    embedder: Arc<dyn Embedder>,
    /// Current search query (write to `/search/query.txt`).
    search_query: Arc<RwLock<String>>,
    /// Cached search results (read from `/search/results.json`).
    search_results: Arc<RwLock<Vec<u8>>>,
    /// Creation time for file attributes.
    creation_time: SystemTime,
}

impl RlmFs {
    /// Creates a new FUSE filesystem wrapping the given storage.
    ///
    /// # Arguments
    ///
    /// * `storage` - The SQLite storage backend.
    /// * `embedder` - The embedder for semantic search.
    #[must_use]
    pub fn new(storage: SqliteStorage, embedder: Arc<dyn Embedder>) -> Self {
        Self {
            storage: Arc::new(RwLock::new(storage)),
            embedder,
            search_query: Arc::new(RwLock::new(String::new())),
            search_results: Arc::new(RwLock::new(b"[]".to_vec())),
            creation_time: SystemTime::now(),
        }
    }

    /// Creates directory attributes.
    fn dir_attr(&self, inode: u64) -> FileAttr {
        FileAttr {
            ino: inode,
            size: 0,
            blocks: 0,
            atime: self.creation_time,
            mtime: self.creation_time,
            ctime: self.creation_time,
            crtime: self.creation_time,
            kind: FileType::Directory,
            perm: 0o755,
            nlink: 2,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }

    /// Creates file attributes with the given size.
    fn file_attr(&self, inode: u64, size: u64) -> FileAttr {
        FileAttr {
            ino: inode,
            size,
            blocks: size.div_ceil(512),
            atime: self.creation_time,
            mtime: self.creation_time,
            ctime: self.creation_time,
            crtime: self.creation_time,
            kind: FileType::RegularFile,
            perm: 0o644,
            nlink: 1,
            uid: unsafe { libc::getuid() },
            gid: unsafe { libc::getgid() },
            rdev: 0,
            blksize: 512,
            flags: 0,
        }
    }

    /// Gets the size of a buffer's content.
    fn get_buffer_size(&self, buffer_id: i64) -> Option<u64> {
        let storage = self.storage.read().ok()?;
        let buffer = storage.get_buffer(buffer_id).ok()??;
        Some(buffer.content.len() as u64)
    }

    /// Gets the size of a chunk's content.
    fn get_chunk_size(&self, chunk_id: i64) -> Option<u64> {
        let storage = self.storage.read().ok()?;
        let chunk = storage.get_chunk(chunk_id).ok()??;
        Some(chunk.content.len() as u64)
    }

    /// Gets the JSON bytes for storage stats.
    fn get_stats_json(&self) -> Vec<u8> {
        let storage = match self.storage.read() {
            Ok(s) => s,
            Err(_) => return b"{}".to_vec(),
        };
        let stats = storage.stats().unwrap_or_default();
        serde_json::to_vec_pretty(&stats).unwrap_or_else(|_| b"{}".to_vec())
    }

    /// Gets the JSON bytes for chunk metadata.
    fn get_chunk_metadata_json(&self, buffer_id: i64) -> Vec<u8> {
        let storage = match self.storage.read() {
            Ok(s) => s,
            Err(_) => return b"[]".to_vec(),
        };
        let chunks = match storage.get_chunks(buffer_id) {
            Ok(c) => c,
            Err(_) => return b"[]".to_vec(),
        };

        let metadata: Vec<_> = chunks
            .iter()
            .map(|c| {
                serde_json::json!({
                    "id": c.id,
                    "index": c.index,
                    "byte_range": {
                        "start": c.byte_range.start,
                        "end": c.byte_range.end
                    },
                    "size": c.size()
                })
            })
            .collect();

        serde_json::to_vec_pretty(&metadata).unwrap_or_else(|_| b"[]".to_vec())
    }

    /// Gets the JSON bytes for an embedding.
    fn get_embedding_json(&self, chunk_id: i64) -> Option<Vec<u8>> {
        let storage = self.storage.read().ok()?;
        let embedding = storage.get_embedding(chunk_id).ok()??;

        let json = serde_json::json!({
            "chunk_id": chunk_id,
            "dimensions": embedding.len(),
            "vector": embedding
        });

        serde_json::to_vec_pretty(&json).ok()
    }

    /// Performs a search and caches the results.
    fn perform_search(&self, query: &str) {
        if query.trim().is_empty() {
            if let Ok(mut results) = self.search_results.write() {
                *results = b"[]".to_vec();
            }
            return;
        }

        let storage = match self.storage.read() {
            Ok(s) => s,
            Err(_) => return,
        };

        let config = SearchConfig::new()
            .with_top_k(20)
            .with_threshold(0.1)
            .with_semantic(true)
            .with_bm25(true);

        let results =
            hybrid_search(&storage, self.embedder.as_ref(), query, &config).unwrap_or_default();

        let json: Vec<_> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "chunk_id": r.chunk_id,
                    "buffer_id": r.buffer_id,
                    "index": r.index,
                    "score": r.score,
                    "semantic_score": r.semantic_score,
                    "bm25_score": r.bm25_score
                })
            })
            .collect();

        if let Ok(mut cached) = self.search_results.write() {
            *cached = serde_json::to_vec_pretty(&json).unwrap_or_else(|_| b"[]".to_vec());
        }
    }
}

impl Filesystem for RlmFs {
    fn lookup(&mut self, _req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name = match name.to_str() {
            Some(n) => n,
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        let inode = match classify_inode(parent) {
            InodeType::Root => match name {
                "buffers" => Some(INODE_BUFFERS_DIR),
                "chunks" => Some(INODE_CHUNKS_DIR),
                "embeddings" => Some(INODE_EMBEDDINGS_DIR),
                "search" => Some(INODE_SEARCH_DIR),
                "stats.json" => Some(INODE_STATS_FILE),
                _ => None,
            },
            InodeType::BuffersDir => parse_buffer_filename(name).map(buffer_id_to_inode),
            InodeType::ChunksDir => {
                // Directory name is buffer ID
                name.parse::<i64>().ok().map(chunk_buffer_dir_to_inode)
            }
            InodeType::ChunkBufferDir(buffer_id) => {
                if name == "metadata.json" {
                    Some(chunk_metadata_to_inode(buffer_id))
                } else if let Some(index) = parse_chunk_filename(name) {
                    // Need to find chunk ID by buffer_id and index
                    let storage = match self.storage.read() {
                        Ok(s) => s,
                        Err(_) => {
                            reply.error(EIO);
                            return;
                        }
                    };
                    match storage.get_chunks(buffer_id) {
                        Ok(chunks) => chunks
                            .iter()
                            .find(|c| c.index == index)
                            .and_then(|c| c.id)
                            .map(chunk_id_to_inode),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            }
            InodeType::EmbeddingsDir => {
                parse_embedding_filename(name).map(embedding_chunk_id_to_inode)
            }
            InodeType::SearchDir => match name {
                "query.txt" => Some(INODE_SEARCH_QUERY),
                "results.json" => Some(INODE_SEARCH_RESULTS),
                _ => None,
            },
            _ => None,
        };

        match inode {
            Some(ino) => {
                let attr = match classify_inode(ino) {
                    InodeType::Root
                    | InodeType::BuffersDir
                    | InodeType::ChunksDir
                    | InodeType::EmbeddingsDir
                    | InodeType::SearchDir => self.dir_attr(ino),
                    InodeType::ChunkBufferDir(_) => self.dir_attr(ino),
                    InodeType::StatsFile => {
                        let json = self.get_stats_json();
                        self.file_attr(ino, json.len() as u64)
                    }
                    InodeType::SearchQuery => {
                        let query = self.search_query.read().map(|q| q.len()).unwrap_or(0);
                        self.file_attr(ino, query as u64)
                    }
                    InodeType::SearchResults => {
                        let results = self.search_results.read().map(|r| r.len()).unwrap_or(2);
                        self.file_attr(ino, results as u64)
                    }
                    InodeType::BufferFile(id) => {
                        let size = self.get_buffer_size(id).unwrap_or(0);
                        self.file_attr(ino, size)
                    }
                    InodeType::ChunkFile(id) => {
                        let size = self.get_chunk_size(id).unwrap_or(0);
                        self.file_attr(ino, size)
                    }
                    InodeType::ChunkMetadata(buffer_id) => {
                        let json = self.get_chunk_metadata_json(buffer_id);
                        self.file_attr(ino, json.len() as u64)
                    }
                    InodeType::EmbeddingFile(chunk_id) => {
                        let json = self.get_embedding_json(chunk_id).unwrap_or_default();
                        self.file_attr(ino, json.len() as u64)
                    }
                    InodeType::Unknown => {
                        reply.error(ENOENT);
                        return;
                    }
                };
                reply.entry(&TTL, &attr, 0);
            }
            None => reply.error(ENOENT),
        }
    }

    fn getattr(&mut self, _req: &Request, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        let attr = match classify_inode(ino) {
            InodeType::Root
            | InodeType::BuffersDir
            | InodeType::ChunksDir
            | InodeType::EmbeddingsDir
            | InodeType::SearchDir => self.dir_attr(ino),
            InodeType::ChunkBufferDir(_) => self.dir_attr(ino),
            InodeType::StatsFile => {
                let json = self.get_stats_json();
                self.file_attr(ino, json.len() as u64)
            }
            InodeType::SearchQuery => {
                let query = self.search_query.read().map(|q| q.len()).unwrap_or(0);
                self.file_attr(ino, query as u64)
            }
            InodeType::SearchResults => {
                let results = self.search_results.read().map(|r| r.len()).unwrap_or(2);
                self.file_attr(ino, results as u64)
            }
            InodeType::BufferFile(id) => {
                let size = self.get_buffer_size(id).unwrap_or(0);
                self.file_attr(ino, size)
            }
            InodeType::ChunkFile(id) => {
                let size = self.get_chunk_size(id).unwrap_or(0);
                self.file_attr(ino, size)
            }
            InodeType::ChunkMetadata(buffer_id) => {
                let json = self.get_chunk_metadata_json(buffer_id);
                self.file_attr(ino, json.len() as u64)
            }
            InodeType::EmbeddingFile(chunk_id) => {
                let json = self.get_embedding_json(chunk_id).unwrap_or_default();
                self.file_attr(ino, json.len() as u64)
            }
            InodeType::Unknown => {
                reply.error(ENOENT);
                return;
            }
        };
        reply.attr(&TTL, &attr);
    }

    fn read(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        let offset = offset as usize;
        let size = size as usize;

        let data: Option<Vec<u8>> = match classify_inode(ino) {
            InodeType::StatsFile => Some(self.get_stats_json()),
            InodeType::SearchQuery => self.search_query.read().ok().map(|q| q.as_bytes().to_vec()),
            InodeType::SearchResults => self.search_results.read().ok().map(|r| r.clone()),
            InodeType::BufferFile(id) => {
                let storage = match self.storage.read() {
                    Ok(s) => s,
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                };
                match storage.get_buffer(id) {
                    Ok(Some(buffer)) => Some(buffer.content.into_bytes()),
                    _ => None,
                }
            }
            InodeType::ChunkFile(id) => {
                let storage = match self.storage.read() {
                    Ok(s) => s,
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                };
                match storage.get_chunk(id) {
                    Ok(Some(chunk)) => Some(chunk.content.into_bytes()),
                    _ => None,
                }
            }
            InodeType::ChunkMetadata(buffer_id) => Some(self.get_chunk_metadata_json(buffer_id)),
            InodeType::EmbeddingFile(chunk_id) => self.get_embedding_json(chunk_id),
            _ => {
                reply.error(ENOENT);
                return;
            }
        };

        match data {
            Some(bytes) => {
                if offset >= bytes.len() {
                    reply.data(&[]);
                } else {
                    let end = (offset + size).min(bytes.len());
                    reply.data(&bytes[offset..end]);
                }
            }
            None => reply.error(ENOENT),
        }
    }

    fn write(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        _offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        match classify_inode(ino) {
            InodeType::SearchQuery => {
                let query = match std::str::from_utf8(data) {
                    Ok(s) => s.trim().to_string(),
                    Err(_) => {
                        reply.error(EINVAL);
                        return;
                    }
                };

                if let Ok(mut cached_query) = self.search_query.write() {
                    *cached_query = query.clone();
                }

                // Perform search
                self.perform_search(&query);

                reply.written(data.len() as u32);
            }
            _ => {
                // Read-only filesystem - only search query is writable
                reply.error(EPERM);
            }
        }
    }

    fn readdir(
        &mut self,
        _req: &Request,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        let mut entries: Vec<(u64, FileType, String)> = vec![
            (ino, FileType::Directory, ".".to_string()),
            (INODE_ROOT, FileType::Directory, "..".to_string()),
        ];

        match classify_inode(ino) {
            InodeType::Root => {
                entries.push((
                    INODE_BUFFERS_DIR,
                    FileType::Directory,
                    "buffers".to_string(),
                ));
                entries.push((INODE_CHUNKS_DIR, FileType::Directory, "chunks".to_string()));
                entries.push((
                    INODE_EMBEDDINGS_DIR,
                    FileType::Directory,
                    "embeddings".to_string(),
                ));
                entries.push((INODE_SEARCH_DIR, FileType::Directory, "search".to_string()));
                entries.push((
                    INODE_STATS_FILE,
                    FileType::RegularFile,
                    "stats.json".to_string(),
                ));
            }
            InodeType::BuffersDir => {
                let storage = match self.storage.read() {
                    Ok(s) => s,
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                };
                match storage.list_buffers() {
                    Ok(buffers) => {
                        for buffer in buffers {
                            if let Some(id) = buffer.id {
                                let ext = buffer
                                    .name
                                    .as_ref()
                                    .and_then(|n| Path::new(n).extension())
                                    .and_then(|e| e.to_str())
                                    .unwrap_or("txt");
                                let name = format!("{id}.{ext}");
                                entries.push((buffer_id_to_inode(id), FileType::RegularFile, name));
                            }
                        }
                    }
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                }
            }
            InodeType::ChunksDir => {
                let storage = match self.storage.read() {
                    Ok(s) => s,
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                };
                match storage.list_buffers() {
                    Ok(buffers) => {
                        for buffer in buffers {
                            if let Some(id) = buffer.id {
                                entries.push((
                                    chunk_buffer_dir_to_inode(id),
                                    FileType::Directory,
                                    id.to_string(),
                                ));
                            }
                        }
                    }
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                }
            }
            InodeType::ChunkBufferDir(buffer_id) => {
                let storage = match self.storage.read() {
                    Ok(s) => s,
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                };
                match storage.get_chunks(buffer_id) {
                    Ok(chunks) => {
                        for chunk in chunks {
                            if let Some(id) = chunk.id {
                                let name = format!("{}.txt", chunk.index);
                                entries.push((chunk_id_to_inode(id), FileType::RegularFile, name));
                            }
                        }
                        // Add metadata.json
                        entries.push((
                            chunk_metadata_to_inode(buffer_id),
                            FileType::RegularFile,
                            "metadata.json".to_string(),
                        ));
                    }
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                }
            }
            InodeType::EmbeddingsDir => {
                let storage = match self.storage.read() {
                    Ok(s) => s,
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                };
                // List all chunks that have embeddings
                match storage.list_buffers() {
                    Ok(buffers) => {
                        for buffer in buffers {
                            if let Some(buffer_id) = buffer.id
                                && let Ok(chunks) = storage.get_chunks(buffer_id)
                            {
                                for chunk in chunks {
                                    if let Some(chunk_id) = chunk.id
                                        && storage.has_embedding(chunk_id).unwrap_or(false)
                                    {
                                        let name = format!("{chunk_id}.json");
                                        entries.push((
                                            embedding_chunk_id_to_inode(chunk_id),
                                            FileType::RegularFile,
                                            name,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        reply.error(EIO);
                        return;
                    }
                }
            }
            InodeType::SearchDir => {
                entries.push((
                    INODE_SEARCH_QUERY,
                    FileType::RegularFile,
                    "query.txt".to_string(),
                ));
                entries.push((
                    INODE_SEARCH_RESULTS,
                    FileType::RegularFile,
                    "results.json".to_string(),
                ));
            }
            _ => {
                reply.error(ENOENT);
                return;
            }
        }

        for (i, (inode, file_type, name)) in entries.into_iter().enumerate().skip(offset as usize) {
            if reply.add(inode, (i + 1) as i64, file_type, name) {
                break;
            }
        }
        reply.ok();
    }

    fn open(&mut self, _req: &Request, ino: u64, flags: i32, reply: fuser::ReplyOpen) {
        // For read-only files, reject write flags (except for search query)
        let write_flags = libc::O_WRONLY | libc::O_RDWR | libc::O_APPEND | libc::O_TRUNC;

        match classify_inode(ino) {
            InodeType::SearchQuery => {
                // Search query is writable
                reply.opened(0, 0);
            }
            InodeType::StatsFile
            | InodeType::SearchResults
            | InodeType::BufferFile(_)
            | InodeType::ChunkFile(_)
            | InodeType::ChunkMetadata(_)
            | InodeType::EmbeddingFile(_) => {
                if flags & write_flags != 0 {
                    reply.error(EPERM);
                } else {
                    reply.opened(0, 0);
                }
            }
            _ => reply.error(ENOENT),
        }
    }

    fn opendir(&mut self, _req: &Request, ino: u64, _flags: i32, reply: fuser::ReplyOpen) {
        match classify_inode(ino) {
            InodeType::Root
            | InodeType::BuffersDir
            | InodeType::ChunksDir
            | InodeType::EmbeddingsDir
            | InodeType::SearchDir
            | InodeType::ChunkBufferDir(_) => {
                reply.opened(0, 0);
            }
            _ => reply.error(ENOENT),
        }
    }

    fn setattr(
        &mut self,
        _req: &Request,
        ino: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        // Only handle truncate for search query
        match classify_inode(ino) {
            InodeType::SearchQuery => {
                if let Some(0) = size {
                    // Truncate query
                    if let Ok(mut query) = self.search_query.write() {
                        query.clear();
                    }
                    if let Ok(mut results) = self.search_results.write() {
                        *results = b"[]".to_vec();
                    }
                }
                let attr = self.file_attr(ino, size.unwrap_or(0));
                reply.attr(&TTL, &attr);
            }
            _ => {
                // Re-fetch current attributes for other files
                self.getattr(_req, ino, None, reply);
            }
        }
    }
}

/// Mounts the FUSE filesystem at the specified path.
///
/// # Arguments
///
/// * `storage` - The SQLite storage backend.
/// * `mountpoint` - The directory to mount the filesystem at.
///
/// # Errors
///
/// Returns an error if mounting fails.
pub fn mount(storage: SqliteStorage, mountpoint: &Path) -> Result<()> {
    let embedder: Arc<dyn Embedder> = create_embedder()?.into();
    let fs = RlmFs::new(storage, embedder);

    let options = vec![
        MountOption::RO,
        MountOption::FSName("rlm-rs".to_string()),
        MountOption::AutoUnmount,
        MountOption::AllowOther,
    ];

    fuser::mount2(fs, mountpoint, &options).map_err(|e| {
        crate::error::CommandError::ExecutionFailed(format!("Failed to mount FUSE: {e}")).into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests would require actual FUSE mounting,
    // which requires root/fuse permissions. Unit tests focus on internal logic.

    #[test]
    fn test_rlmfs_creation() {
        // This test just verifies the struct can be constructed
        // Full tests would require a test database
    }
}
