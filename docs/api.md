# RLM-RS API Reference

Rust library API documentation for `rlm-rs`.

## Overview

`rlm-rs` can be used as both a CLI tool and a Rust library. This document covers the library API for programmatic integration.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
rlm-rs = "1.1"
```

Basic usage:

```rust
use rlm_rs::{Buffer, Chunker, SemanticChunker, SqliteStorage, Storage};

fn main() -> rlm_rs::Result<()> {
    // Initialize storage
    let mut storage = SqliteStorage::open(".rlm/rlm-state.db")?;
    storage.init()?;

    // Create a buffer from content
    let buffer = Buffer::from_content("Hello, world!".to_string());
    let buffer_id = storage.add_buffer(&buffer)?;

    // Chunk the content
    let chunker = SemanticChunker::new();
    let chunks = chunker.chunk(buffer_id, &buffer.content, None)?;

    // Store chunks
    storage.add_chunks(buffer_id, &chunks)?;

    Ok(())
}
```

---

## Core Types

### `Buffer`

Represents a text buffer loaded into the RLM system.

**Location:** `rlm_rs::core::Buffer`

```rust
pub struct Buffer {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub source: Option<PathBuf>,
    pub content: String,
    pub metadata: BufferMetadata,
}
```

#### Constructors

| Method | Description |
|--------|-------------|
| `Buffer::from_content(content: String)` | Create from string content |
| `Buffer::from_file(path: PathBuf, content: String)` | Create from file path and content |
| `Buffer::from_named(name: String, content: String)` | Create with explicit name |

#### Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `size()` | `usize` | Buffer size in bytes |
| `line_count()` | `usize` | Number of lines (cached) |
| `slice(start, end)` | `Option<&str>` | Get content slice |
| `peek(len)` | `&str` | Preview first N bytes |
| `peek_end(len)` | `&str` | Preview last N bytes |
| `is_empty()` | `bool` | Check if empty |
| `display_name()` | `String` | Human-readable name |
| `compute_hash()` | `()` | Compute content hash |

#### Example

```rust
use rlm_rs::Buffer;
use std::path::PathBuf;

// From content
let buffer = Buffer::from_content("Hello, world!".to_string());
assert_eq!(buffer.size(), 13);

// From file
let buffer = Buffer::from_file(
    PathBuf::from("document.md"),
    std::fs::read_to_string("document.md")?,
);
assert!(buffer.source.is_some());

// Slicing
if let Some(slice) = buffer.slice(0, 100) {
    println!("First 100 bytes: {}", slice);
}
```

---

### `BufferMetadata`

Metadata associated with a buffer.

```rust
pub struct BufferMetadata {
    pub content_type: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub size: usize,
    pub line_count: Option<usize>,
    pub chunk_count: Option<usize>,
    pub content_hash: Option<String>,
}
```

---

### `Chunk`

Represents a segment of buffer content.

**Location:** `rlm_rs::core::Chunk`

```rust
pub struct Chunk {
    pub id: Option<i64>,
    pub buffer_id: i64,
    pub content: String,
    pub byte_range: Range<usize>,
    pub index: usize,
    pub metadata: ChunkMetadata,
}
```

#### Constructors

| Method | Description |
|--------|-------------|
| `Chunk::new(buffer_id, content, byte_range, index)` | Create new chunk |
| `Chunk::with_strategy(buffer_id, content, byte_range, index, strategy)` | Create with strategy name |
| `ChunkBuilder::new()` | Fluent builder pattern |

#### Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `size()` | `usize` | Chunk content size in bytes |
| `range_size()` | `usize` | Byte range size |
| `start()` | `usize` | Start byte offset |
| `end()` | `usize` | End byte offset |
| `is_empty()` | `bool` | Check if empty |
| `estimate_tokens()` | `usize` | Estimate token count (~4 chars/token) |
| `preview(max_len)` | `&str` | Preview first N characters |
| `overlaps_with(range)` | `bool` | Check if overlaps with range |
| `contains_offset(offset)` | `bool` | Check if contains byte offset |
| `compute_hash()` | `()` | Compute content hash |

#### Example

```rust
use rlm_rs::Chunk;

let chunk = Chunk::new(
    1,                          // buffer_id
    "Hello, world!".to_string(), // content
    0..13,                       // byte_range
    0,                           // index
);

assert_eq!(chunk.size(), 13);
assert_eq!(chunk.estimate_tokens(), 4); // ~4 chars per token
assert!(chunk.contains_offset(5));
```

#### Builder Pattern

```rust
use rlm_rs::core::chunk::ChunkBuilder;

let chunk = ChunkBuilder::new()
    .buffer_id(1)
    .content("Hello, world!".to_string())
    .byte_range(0..13)
    .index(0)
    .strategy("semantic")
    .has_overlap(false)
    .build();
```

---

### `ChunkMetadata`

Metadata associated with a chunk.

```rust
pub struct ChunkMetadata {
    pub strategy: Option<String>,
    pub token_count: Option<usize>,
    pub line_range: Option<Range<usize>>,
    pub created_at: i64,
    pub content_hash: Option<String>,
    pub has_overlap: bool,
    pub custom: Option<String>,
}
```

---

### `Context`

Manages variables and state.

**Location:** `rlm_rs::core::Context`

```rust
pub struct Context {
    pub variables: HashMap<String, ContextValue>,
    pub globals: HashMap<String, ContextValue>,
    pub buffer_ids: Vec<i64>,
    pub cwd: Option<String>,
    pub metadata: ContextMetadata,
}
```

---

### `ContextValue`

Typed values for context variables.

```rust
pub enum ContextValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<ContextValue>),
    Map(HashMap<String, ContextValue>),
    Null,
}
```

---

## Chunking

### `Chunker` Trait

All chunking strategies implement this trait.

**Location:** `rlm_rs::chunking::Chunker`

```rust
pub trait Chunker: Send + Sync {
    fn chunk(
        &self,
        buffer_id: i64,
        text: &str,
        metadata: Option<&ChunkMetadata>,
    ) -> Result<Vec<Chunk>>;

    fn name(&self) -> &'static str;
    fn supports_parallel(&self) -> bool;
    fn description(&self) -> &'static str;
    fn validate(&self, metadata: Option<&ChunkMetadata>) -> Result<()>;
}
```

### Chunking Strategies

#### `SemanticChunker`

Unicode-aware chunking that respects sentence and paragraph boundaries.

```rust
use rlm_rs::chunking::{Chunker, SemanticChunker};

let chunker = SemanticChunker::new();
// Or with custom size:
let chunker = SemanticChunker::with_size(20_000);
// Or with size and overlap:
let chunker = SemanticChunker::with_size_and_overlap(20_000, 500);

let chunks = chunker.chunk(1, "Your long text...", None)?;
```

**Best for:** Markdown, prose, code, structured documents.

---

#### `FixedChunker`

Simple character-based chunking at exact boundaries.

```rust
use rlm_rs::chunking::{Chunker, FixedChunker};

let chunker = FixedChunker::new();
// Or with custom size:
let chunker = FixedChunker::with_size(50_000);
// Or with size and overlap:
let chunker = FixedChunker::with_size_and_overlap(50_000, 1000);

let chunks = chunker.chunk(1, "Your long text...", None)?;
```

**Best for:** Logs, plain text, binary-safe content.

---

#### `ParallelChunker`

Multi-threaded chunking using Rayon for large files.

```rust
use rlm_rs::chunking::{Chunker, ParallelChunker, SemanticChunker};

let inner = SemanticChunker::new();
let chunker = ParallelChunker::new(inner);

let chunks = chunker.chunk(1, "Your very large text...", None)?;
```

**Best for:** Large files (>10MB).

---

### Factory Functions

```rust
use rlm_rs::chunking::{create_chunker, available_strategies};

// Create chunker by name
let chunker = create_chunker("semantic")?;
let chunker = create_chunker("fixed")?;
let chunker = create_chunker("parallel")?;

// List available strategies
let strategies = available_strategies(); // ["fixed", "semantic", "parallel"]
```

---

### Chunking Configuration

```rust
use rlm_rs::chunking::traits::ChunkMetadata;

let metadata = ChunkMetadata::new()
    .with_size_and_overlap(30_000, 500)
    .source("document.md")
    .content_type("md")
    .preserve_sentences(true)
    .max_chunks(100);

let chunks = chunker.chunk(1, text, Some(&metadata))?;
```

---

### Constants

```rust
use rlm_rs::chunking::{DEFAULT_CHUNK_SIZE, DEFAULT_OVERLAP, MAX_CHUNK_SIZE};

// DEFAULT_CHUNK_SIZE = 3_000 (~750 tokens)
// DEFAULT_OVERLAP = 500
// MAX_CHUNK_SIZE = 50_000
```

---

## Storage

### `Storage` Trait

Interface for persistent storage backends.

**Location:** `rlm_rs::storage::Storage`

```rust
pub trait Storage: Send + Sync {
    // Lifecycle
    fn init(&mut self) -> Result<()>;
    fn is_initialized(&self) -> Result<bool>;
    fn reset(&mut self) -> Result<()>;

    // Context
    fn save_context(&mut self, context: &Context) -> Result<()>;
    fn load_context(&self) -> Result<Option<Context>>;
    fn delete_context(&mut self) -> Result<()>;

    // Buffers
    fn add_buffer(&mut self, buffer: &Buffer) -> Result<i64>;
    fn get_buffer(&self, id: i64) -> Result<Option<Buffer>>;
    fn get_buffer_by_name(&self, name: &str) -> Result<Option<Buffer>>;
    fn list_buffers(&self) -> Result<Vec<Buffer>>;
    fn update_buffer(&mut self, buffer: &Buffer) -> Result<()>;
    fn delete_buffer(&mut self, id: i64) -> Result<()>;
    fn buffer_count(&self) -> Result<usize>;

    // Chunks
    fn add_chunks(&mut self, buffer_id: i64, chunks: &[Chunk]) -> Result<()>;
    fn get_chunks(&self, buffer_id: i64) -> Result<Vec<Chunk>>;
    fn get_chunk(&self, id: i64) -> Result<Option<Chunk>>;
    fn delete_chunks(&mut self, buffer_id: i64) -> Result<()>;
    fn chunk_count(&self, buffer_id: i64) -> Result<usize>;

    // Utilities
    fn export_buffers(&self) -> Result<String>;
    fn stats(&self) -> Result<StorageStats>;
}
```

---

### `SqliteStorage`

SQLite-backed storage implementation.

**Location:** `rlm_rs::storage::SqliteStorage`

```rust
use rlm_rs::{SqliteStorage, Storage};

// Open or create database
let mut storage = SqliteStorage::open(".rlm/rlm-state.db")?;

// Initialize schema
storage.init()?;

// Check if initialized
if storage.is_initialized()? {
    println!("Database ready");
}

// Get statistics
let stats = storage.stats()?;
println!("Buffers: {}", stats.buffer_count);
println!("Chunks: {}", stats.chunk_count);
```

---

### `StorageStats`

Storage statistics.

```rust
pub struct StorageStats {
    pub buffer_count: usize,
    pub chunk_count: usize,
    pub total_content_size: usize,
    pub has_context: bool,
    pub schema_version: u32,
    pub db_size: Option<u64>,
}
```

---

### Vector Search (Feature-Gated)

Enable with `vector-search` feature:

```toml
[dependencies]
rlm-rs = { version = "1.1", features = ["full-search"] }
```

```rust
#[cfg(feature = "vector-search")]
use rlm_rs::storage::VectorStorage;

// Index a chunk with embeddings
storage.index_chunk(chunk_id, &embedding_vector)?;

// Search for similar chunks
let results = storage.search_similar(&query_embedding, 10)?;
for (chunk_id, score) in results {
    println!("Chunk {}: similarity {}", chunk_id, score);
}
```

---

## I/O

### File Reading

**Location:** `rlm_rs::io`

```rust
use rlm_rs::io::{read_file, read_file_mmap};
use std::path::Path;

// Standard file read
let content = read_file(Path::new("document.md"))?;

// Memory-mapped read (efficient for large files)
let content = read_file_mmap(Path::new("large-file.txt"))?;
```

---

### File Writing

```rust
use rlm_rs::io::{write_file, write_chunks};
use std::path::Path;

// Write content to file
write_file(Path::new("output.txt"), "content")?;

// Write chunks to directory
write_chunks(
    Path::new(".rlm/chunks"),
    &chunks,
    "chunk",  // prefix
)?;
// Creates: chunk_0.txt, chunk_1.txt, ...
```

---

### Unicode Utilities

```rust
use rlm_rs::io::{find_char_boundary, validate_utf8};

// Find valid UTF-8 boundary at or before position
let boundary = find_char_boundary("Hello, 世界!", 8);

// Validate UTF-8
validate_utf8(bytes)?;
```

---

## Error Handling

### Error Types

**Location:** `rlm_rs::error`

```rust
use rlm_rs::{Error, Result};

pub enum Error {
    Storage(StorageError),
    Chunking(ChunkingError),
    Io(IoError),
    Command(CommandError),
    InvalidState { message: String },
    Config { message: String },
}
```

### `StorageError`

```rust
pub enum StorageError {
    Database(String),
    NotInitialized,
    ContextNotFound,
    BufferNotFound { identifier: String },
    ChunkNotFound { id: i64 },
    Migration(String),
    Transaction(String),
    Serialization(String),
}
```

### `ChunkingError`

```rust
pub enum ChunkingError {
    InvalidUtf8 { offset: usize },
    ChunkTooLarge { size: usize, max: usize },
    InvalidConfig { reason: String },
    OverlapTooLarge { overlap: usize, size: usize },
    ParallelFailed { reason: String },
    SemanticFailed(String),
    Regex(String),
    UnknownStrategy { name: String },
}
```

### `IoError`

```rust
pub enum IoError {
    FileNotFound { path: String },
    ReadFailed { path: String, reason: String },
    WriteFailed { path: String, reason: String },
    MmapFailed { path: String, reason: String },
    DirectoryFailed { path: String, reason: String },
    PathTraversal { path: String },
    Generic(String),
}
```

---

## CLI Integration

### Using the CLI Types

```rust
use rlm_rs::cli::{Cli, Commands, execute};
use clap::Parser;

// Parse arguments
let cli = Cli::parse();

// Execute command
let output = execute(&cli)?;
println!("{}", output);
```

### Output Formats

```rust
use rlm_rs::cli::OutputFormat;

match format {
    OutputFormat::Text => println!("{}", result),
    OutputFormat::Json => println!("{}", serde_json::to_string(&result)?),
}
```

---

## Re-exports

The crate root re-exports commonly used types:

```rust
// Error handling
pub use error::{Error, Result};

// Core types
pub use core::{Buffer, BufferMetadata, Chunk, ChunkMetadata, Context, ContextValue};

// Storage
pub use storage::{DEFAULT_DB_PATH, SqliteStorage, Storage};

// Chunking
pub use chunking::{Chunker, FixedChunker, SemanticChunker, available_strategies, create_chunker};

// CLI
pub use cli::{Cli, Commands, OutputFormat};
```

---

## Complete Example

```rust
use rlm_rs::{
    Buffer, Chunk, Chunker, SemanticChunker, SqliteStorage, Storage, Result,
};
use std::path::PathBuf;

fn process_document(path: &str) -> Result<()> {
    // 1. Initialize storage
    let mut storage = SqliteStorage::open(".rlm/rlm-state.db")?;
    storage.init()?;

    // 2. Read file content
    let content = std::fs::read_to_string(path)?;

    // 3. Create buffer
    let buffer = Buffer::from_file(PathBuf::from(path), content.clone());
    let buffer_id = storage.add_buffer(&buffer)?;

    // 4. Chunk the content
    let chunker = SemanticChunker::with_size_and_overlap(3_000, 500);
    let chunks = chunker.chunk(buffer_id, &content, None)?;

    println!("Created {} chunks", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!(
            "  Chunk {}: {} bytes, ~{} tokens",
            i,
            chunk.size(),
            chunk.estimate_tokens()
        );
    }

    // 5. Store chunks
    storage.add_chunks(buffer_id, &chunks)?;

    // 6. Query stored data
    let stats = storage.stats()?;
    println!("\nStorage stats:");
    println!("  Buffers: {}", stats.buffer_count);
    println!("  Chunks: {}", stats.chunk_count);
    println!("  Total size: {} bytes", stats.total_content_size);

    Ok(())
}
```

---

## See Also

- [README.md](../README.md) - Project overview
- [Architecture](architecture.md) - Internal architecture
- [CLI Reference](cli-reference.md) - Command-line interface
- [docs.rs/rlm-rs](https://docs.rs/rlm-rs) - Auto-generated rustdoc
