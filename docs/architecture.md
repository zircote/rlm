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
│  ┌────────────┬────────────┴────────────┬────────────┬────────────┐│
│  │  Chunking  │       Storage           │ Embedding  │    I/O     ││
│  │  ─────────  │       ───────           │ ─────────  │    ───     ││
│  │  Fixed     │       SQLite            │  BGE-M3   │   Reader   ││
│  │  Semantic  │       FTS5 (BM25)       │ fastembed │   (mmap)   ││
│  │  Code      │       Hybrid Search     │  (1024d)  │   Unicode  ││
│  │  Parallel  │       HNSW (optional)   │           │            ││
│  └────────────┴─────────────────────────┴────────────┴────────────┘│
└─────────────────────────────────────────────────────────────────────┘
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
│   ├── context.rs   # Context: variables and state
│   └── relevance.rs # Relevance enum (shared across agent and CLI)
│
├── chunking/        # Chunking strategies
│   ├── mod.rs       # Strategy factory and constants
│   ├── traits.rs    # Chunker trait definition
│   ├── fixed.rs     # Fixed-size chunking
│   ├── semantic.rs  # Sentence/paragraph-aware chunking
│   ├── code.rs      # Language-aware code chunking
│   └── parallel.rs  # Multi-threaded chunking
│
├── embedding/       # Embedding generation
│   ├── mod.rs       # Embedding trait and constants
│   ├── fastembed_impl.rs  # BGE-M3 via fastembed-rs
│   └── fallback.rs  # Fallback when fastembed unavailable
│
├── search/          # Hybrid search
│   ├── mod.rs       # Search orchestration and RRF fusion
│   ├── rrf.rs       # Reciprocal Rank Fusion algorithm
│   └── hnsw.rs      # HNSW vector index (optional)
│
├── storage/         # Persistence layer
│   ├── mod.rs
│   ├── traits.rs    # Storage trait definition
│   ├── sqlite.rs    # SQLite implementation
│   └── schema.rs    # Schema migrations
│
├── io/              # File I/O
│   ├── mod.rs
│   ├── reader.rs    # File reading with mmap
│   └── unicode.rs   # Unicode/grapheme utilities
│
├── cli/             # Command-line interface
│   ├── mod.rs
│   ├── parser.rs    # Clap argument definitions
│   ├── commands.rs  # Command implementations
│   └── output.rs    # Output formatting
│
└── agent/           # Agentic query system (feature: "agent")
    ├── mod.rs
    ├── config.rs    # Agent configuration and builder
    ├── orchestrator.rs  # Fan-out/collect pipeline
    ├── primary.rs   # Primary analysis agent
    ├── subcall.rs   # Chunk analysis subcall agent
    ├── synthesizer.rs   # Finding synthesis agent
    ├── executor.rs  # Tool-calling executor
    ├── tool.rs      # Tool schema definitions
    ├── prompt.rs    # Prompt assembly
    ├── provider.rs  # LLM provider trait
    ├── client.rs    # OpenAI-compatible client
    ├── agentic_loop.rs  # Tool-calling iteration loop
    ├── finding.rs   # Finding and result types
    ├── message.rs   # Message and token types
    └── traits.rs    # Agent trait definition
```

## Core Types

### Buffer

Represents a loaded file with metadata:

```rust
pub struct Buffer {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub source: Option<PathBuf>,
    pub content: String,
    pub metadata: BufferMetadata,
}

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

### Chunk

Represents a segment of buffer content:

```rust
pub struct Chunk {
    pub id: Option<i64>,
    pub buffer_id: i64,
    pub content: String,
    pub byte_range: Range<usize>,
    pub index: usize,
    pub metadata: ChunkMetadata,
}

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

### Context

Manages variables and state:

```rust
pub struct Context {
    pub variables: HashMap<String, ContextValue>,
    pub globals: HashMap<String, ContextValue>,
    pub buffer_ids: Vec<i64>,
    pub cwd: Option<String>,
    pub metadata: ContextMetadata,
}

pub enum ContextValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<Self>),
    Map(HashMap<String, Self>),
    Null,
}

pub struct ContextMetadata {
    pub created_at: i64,
    pub updated_at: i64,
    pub version: u32,
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
| `SemanticChunker` | Unicode sentence/paragraph boundaries | Markdown, prose |
| `CodeChunker` | Language-aware function/class boundaries | Source code files |
| `FixedChunker` | Character boundaries with UTF-8 safety | Logs, raw text |
| `ParallelChunker` | Rayon-parallelized fixed chunking | Large files (>10MB) |

### Code Chunker Languages

The `CodeChunker` uses regex-based pattern matching for multiple languages:

| Language | Extensions | Boundary Detection |
|----------|------------|-------------------|
| Rust | .rs | `fn`, `impl`, `struct`, `enum`, `mod` |
| Python | .py | `def`, `class`, `async def` |
| JavaScript/TypeScript | .js, .jsx, .ts, .tsx | `function`, `class`, `const =` |
| Go | .go | `func`, `type` |
| Java | .java | `class`, `interface`, method signatures |
| C/C++ | .c, .cpp, .h, .hpp | Function definitions |
| Ruby | .rb | `def`, `class`, `module` |
| PHP | .php | `function`, `class` |

### Default Configuration

```rust
pub const DEFAULT_CHUNK_SIZE: usize = 3_000;    // ~750 tokens
pub const DEFAULT_OVERLAP: usize = 500;          // Context continuity
pub const MAX_CHUNK_SIZE: usize = 50_000;        // Safety limit
```

## Storage Layer

### Storage Trait

```rust
pub trait Storage {
    // Lifecycle
    fn init(&mut self) -> Result<()>;
    fn is_initialized(&self) -> Result<bool>;
    fn reset(&mut self) -> Result<()>;

    // Context operations
    fn save_context(&mut self, context: &Context) -> Result<()>;
    fn load_context(&self) -> Result<Option<Context>>;
    fn delete_context(&mut self) -> Result<()>;

    // Buffer operations
    fn add_buffer(&mut self, buffer: &Buffer) -> Result<i64>;
    fn get_buffer(&self, id: i64) -> Result<Option<Buffer>>;
    fn get_buffer_by_name(&self, name: &str) -> Result<Option<Buffer>>;
    fn list_buffers(&self) -> Result<Vec<Buffer>>;
    fn update_buffer(&mut self, buffer: &Buffer) -> Result<()>;
    fn delete_buffer(&mut self, id: i64) -> Result<()>;
    fn buffer_count(&self) -> Result<usize>;

    // Chunk operations
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
pub const fn find_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let bytes = s.as_bytes();
    let mut boundary = pos;
    // UTF-8 continuation bytes start with 10xxxxxx (0x80-0xBF)
    while boundary > 0 && (bytes[boundary] & 0xC0) == 0x80 {
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
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("chunking error: {0}")]
    Chunking(#[from] ChunkingError),

    #[error("I/O error: {0}")]
    Io(#[from] IoError),

    #[error("command error: {0}")]
    Command(#[from] CommandError),

    #[error("search error: {0}")]
    Search(#[from] SearchError),

    #[error("invalid state: {message}")]
    InvalidState { message: String },

    #[error("configuration error: {message}")]
    Config { message: String },

    #[cfg(feature = "agent")]
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),
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

Chunks target ~750 tokens (3,000 characters) for optimal semantic search granularity:

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

## Search System

### Hybrid Search Architecture

rlm-rs implements a hybrid search system combining multiple retrieval methods:

```
┌─────────────────────────────────────────────────────────────┐
│                      Search Query                            │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
    ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
    │ Semantic Search │ │  BM25 Search    │ │  HNSW Index     │
    │  (Embeddings)   │ │  (FTS5)         │ │  (Optional)     │
    └────────┬────────┘ └────────┬────────┘ └────────┬────────┘
             │                   │                   │
             └───────────────────┼───────────────────┘
                                 ▼
                    ┌───────────────────────┐
                    │  Reciprocal Rank      │
                    │  Fusion (RRF)         │
                    └───────────────────────┘
                                 │
                                 ▼
                    ┌───────────────────────┐
                    │   Ranked Results      │
                    └───────────────────────┘
```

### Embedding System

| Component | Implementation | Details |
|-----------|---------------|---------|
| Model | BGE-M3 via fastembed | 1024 dimensions |
| Fallback | Hash-based embedder | When fastembed unavailable |
| Storage | SQLite BLOB | Compact binary storage |
| Incremental | `embed_buffer_chunks_incremental` | Only new/changed chunks |

### HNSW Index (Optional)

When the `usearch-hnsw` feature is enabled:

- O(log n) approximate nearest neighbor search
- Persistent index on disk
- Incremental updates
- Falls back to brute-force when disabled

## Future Extensions

### Planned Features

- **Compression**: Compress stored content
- **Encryption**: Encrypt sensitive buffers

### Extension Points

- `Chunker` trait for custom chunking strategies
- `Embedder` trait for alternative embedding models
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
