# RLM-RS Architecture

Internal architecture documentation for `rlm-rs`.

## Overview

RLM-RS implements the Recursive Language Model (RLM) pattern from [arXiv:2512.24601](https://arxiv.org/abs/2512.24601), enabling LLMs to process documents up to 100x larger than their context windows.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        Claude Code                               │
│  ┌─────────────────┐    ┌─────────────────┐                     │
│  │   Root LLM      │───▶│   Sub-LLM       │                     │
│  │ (Opus/Sonnet)   │    │   (Haiku)       │                     │
│  └────────┬────────┘    └────────┬────────┘                     │
│           │                      │                               │
│           ▼                      ▼                               │
│  ┌─────────────────────────────────────────┐                    │
│  │              Bash Tool                   │                    │
│  └─────────────────┬───────────────────────┘                    │
└────────────────────┼────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────────┐
│                        rlm-rs CLI                                │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                      CLI Layer                               ││
│  │  parser.rs │ commands.rs │ output.rs                        ││
│  └─────────────────────────┬───────────────────────────────────┘│
│                            │                                     │
│  ┌─────────────────────────┴───────────────────────────────────┐│
│  │                    Core Domain                               ││
│  │  Buffer │ Chunk │ Context │ Variable                        ││
│  └─────────────────────────┬───────────────────────────────────┘│
│                            │                                     │
│  ┌────────────┬────────────┴────────────┬────────────┐         │
│  │  Chunking  │       Storage           │    I/O     │         │
│  │  ─────────  │       ───────           │    ───     │         │
│  │  Fixed     │       SQLite            │   Reader   │         │
│  │  Semantic  │       (rusqlite)        │   (mmap)   │         │
│  │  Parallel  │                         │   Unicode  │         │
│  └────────────┴─────────────────────────┴────────────┘         │
└─────────────────────────────────────────────────────────────────┘
```

## Module Structure

```
src/
├── lib.rs           # Library entry point and public API
├── main.rs          # Binary entry point
├── error.rs         # Error types (thiserror)
│
├── core/            # Core domain types
│   ├── mod.rs
│   ├── buffer.rs    # Buffer: loaded file content
│   ├── chunk.rs     # Chunk: content segment with metadata
│   └── context.rs   # Context: variables and state
│
├── chunking/        # Chunking strategies
│   ├── mod.rs       # Strategy factory and constants
│   ├── traits.rs    # Chunker trait definition
│   ├── fixed.rs     # Fixed-size chunking
│   ├── semantic.rs  # Sentence/paragraph-aware chunking
│   └── parallel.rs  # Multi-threaded chunking
│
├── storage/         # Persistence layer
│   ├── mod.rs
│   ├── traits.rs    # Storage trait definition
│   └── sqlite.rs    # SQLite implementation
│
├── io/              # File I/O
│   ├── mod.rs
│   ├── reader.rs    # File reading with mmap
│   └── unicode.rs   # Unicode/grapheme utilities
│
└── cli/             # Command-line interface
    ├── mod.rs
    ├── parser.rs    # Clap argument definitions
    ├── commands.rs  # Command implementations
    └── output.rs    # Output formatting
```

## Core Types

### Buffer

Represents a loaded file with metadata:

```rust
pub struct Buffer {
    pub id: Option<i64>,
    pub name: String,
    pub content: String,
    pub source: Option<String>,
    pub metadata: BufferMetadata,
}

pub struct BufferMetadata {
    pub size: usize,
    pub line_count: usize,
    pub hash: String,
    pub content_type: Option<String>,
    pub chunk_count: usize,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}
```

### Chunk

Represents a segment of buffer content:

```rust
pub struct Chunk {
    pub buffer_id: i64,
    pub content: String,
    pub byte_range: Range<usize>,
    pub index: usize,
    pub metadata: ChunkMetadata,
}

pub struct ChunkMetadata {
    pub token_count: Option<usize>,
    pub has_overlap: bool,
    pub strategy: Option<String>,
}
```

### Context

Manages variables and state:

```rust
pub struct Context {
    buffers: HashMap<i64, Buffer>,
    variables: HashMap<String, ContextValue>,
    globals: HashMap<String, ContextValue>,
}

pub enum ContextValue {
    String(String),
    Number(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<ContextValue>),
}
```

## Chunking System

### Chunker Trait

All chunking strategies implement:

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

### Strategy Selection

| Strategy | Algorithm | Use Case |
|----------|-----------|----------|
| `SemanticChunker` | Unicode sentence/paragraph boundaries | Markdown, prose, code |
| `FixedChunker` | Character boundaries with UTF-8 safety | Logs, raw text |
| `ParallelChunker` | Rayon-parallelized fixed chunking | Large files (>10MB) |

### Default Configuration

```rust
pub const DEFAULT_CHUNK_SIZE: usize = 40_000;   // ~10k tokens
pub const DEFAULT_OVERLAP: usize = 500;          // Context continuity
pub const MAX_CHUNK_SIZE: usize = 250_000;       // Safety limit
```

## Storage Layer

### Storage Trait

```rust
pub trait Storage: Send + Sync {
    // Buffer operations
    fn add_buffer(&mut self, buffer: &Buffer) -> Result<i64>;
    fn get_buffer(&self, id: i64) -> Result<Option<Buffer>>;
    fn get_buffer_by_name(&self, name: &str) -> Result<Option<Buffer>>;
    fn update_buffer(&mut self, buffer: &Buffer) -> Result<()>;
    fn delete_buffer(&mut self, id: i64) -> Result<()>;
    fn list_buffers(&self) -> Result<Vec<Buffer>>;

    // Chunk operations
    fn add_chunks(&mut self, buffer_id: i64, chunks: &[Chunk]) -> Result<()>;
    fn get_chunks(&self, buffer_id: i64) -> Result<Vec<Chunk>>;
    fn delete_chunks(&mut self, buffer_id: i64) -> Result<()>;

    // Variable operations
    fn set_variable(&mut self, name: &str, value: &ContextValue) -> Result<()>;
    fn get_variable(&self, name: &str) -> Result<Option<ContextValue>>;
    fn delete_variable(&mut self, name: &str) -> Result<()>;

    // Global operations
    fn set_global(&mut self, name: &str, value: &ContextValue) -> Result<()>;
    fn get_global(&self, name: &str) -> Result<Option<ContextValue>>;
    fn delete_global(&mut self, name: &str) -> Result<()>;
}
```

### SQLite Schema

```sql
-- Buffers table
CREATE TABLE buffers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    source TEXT,
    size INTEGER NOT NULL,
    line_count INTEGER NOT NULL,
    hash TEXT NOT NULL,
    content_type TEXT,
    chunk_count INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Chunks table
CREATE TABLE chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    buffer_id INTEGER NOT NULL REFERENCES buffers(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    chunk_index INTEGER NOT NULL,
    token_count INTEGER,
    has_overlap INTEGER DEFAULT 0,
    strategy TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Variables table
CREATE TABLE variables (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

-- Globals table
CREATE TABLE globals (
    name TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

## I/O Layer

### Memory-Mapped File Reading

For large files, `memmap2` provides efficient reading:

```rust
pub fn read_file(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let content = std::str::from_utf8(&mmap)?;
    Ok(content.to_string())
}
```

### Unicode Handling

The `unicode-segmentation` crate ensures proper handling of:
- Multi-byte UTF-8 characters
- Grapheme clusters
- Sentence boundaries

```rust
pub fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut boundary = pos;
    while !s.is_char_boundary(boundary) && boundary > 0 {
        boundary -= 1;
    }
    boundary
}
```

## Error Handling

All errors use `thiserror` for ergonomic error types:

```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Chunking error: {0}")]
    Chunking(#[from] ChunkingError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Command error: {0}")]
    Command(#[from] CommandError),
}
```

## RLM Pattern Implementation

### Concept Mapping

| RLM Concept | rlm-rs Implementation |
|-------------|----------------------|
| Root LLM | Claude Code main conversation (Opus/Sonnet) |
| Sub-LLM | Claude Code subagent (Haiku) |
| External Environment | `rlm-rs` CLI + SQLite database |
| Chunk | `Chunk` struct with byte range and metadata |
| Buffer | `Buffer` struct with full content |
| State | SQLite persistence + context variables |

### Workflow

1. **Load**: Large document loaded into buffer, chunked, stored in SQLite
2. **Index**: Root LLM queries chunk indices via `chunk-indices`
3. **Process**: Sub-LLM processes individual chunks via file reads
4. **Aggregate**: Results stored back via `add-buffer`
5. **Synthesize**: Root LLM synthesizes final result

## Performance Considerations

### Token Estimation

Chunks target ~10,000 tokens to fit within Claude's 25,000 token read limit:

```rust
impl Chunk {
    pub fn estimate_tokens(&self) -> usize {
        // Approximate: 4 characters per token
        self.content.len() / 4
    }
}
```

### Parallel Processing

The `ParallelChunker` uses Rayon for multi-threaded chunking:

```rust
impl Chunker for ParallelChunker {
    fn chunk(&self, buffer_id: i64, text: &str, metadata: Option<&ChunkMetadata>) -> Result<Vec<Chunk>> {
        let segments = split_into_segments(text, self.segment_count);

        segments
            .par_iter()
            .enumerate()
            .flat_map(|(i, segment)| {
                self.inner.chunk(buffer_id, segment, metadata)
            })
            .collect()
    }
}
```

## Testing Strategy

### Unit Tests

Each module has `#[cfg(test)]` tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_boundaries() {
        let chunker = SemanticChunker::with_size(100);
        let chunks = chunker.chunk(1, "Hello. World.", None).unwrap();
        assert!(!chunks.is_empty());
    }
}
```

### Integration Tests

`tests/integration_test.rs` covers end-to-end workflows.

### Property-Based Tests

Using `proptest` for invariant verification:

```rust
proptest! {
    #[test]
    fn chunk_byte_range_valid(content in ".{1,1000}") {
        let chunker = FixedChunker::with_size(100);
        let chunks = chunker.chunk(1, &content, None).unwrap();
        for chunk in chunks {
            prop_assert!(chunk.byte_range.end <= content.len());
        }
    }
}
```

## Future Extensions

### Planned Features

- **Vector Search**: Semantic similarity search via `sqlite-vec`
- **Streaming**: Process chunks as they're generated
- **Compression**: Compress stored content
- **Encryption**: Encrypt sensitive buffers

### Extension Points

- `Chunker` trait for custom chunking strategies
- `Storage` trait for alternative backends (PostgreSQL, Redis)
- Output formatters for additional formats (YAML, TOML)

---

## See Also

- [RLM-Inspired Design](rlm-inspired-design.md) - How rlm-rs builds on the RLM paper
- [Plugin Integration](plugin-integration.md) - Claude Code plugin setup and portability
- [CLI Reference](cli-reference.md) - Complete command documentation
- [API Reference](api.md) - Rust library documentation
- [README.md](../README.md) - Project overview
- [RLM Paper](https://arxiv.org/abs/2512.24601) - Original research paper
