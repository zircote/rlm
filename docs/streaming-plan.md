# RLM-RS Streaming Input Architecture Plan

## Overview

This plan explores options for adding streaming input capabilities to rlm-rs, enabling processing of unbounded data streams without loading entire files into memory.

**User Requirements:**
- Primary use case: CLI pipeline integration (`cat | rlm-rs | grep`)
- Input sources: stdin, large files, AND network/API streams
- Exploration of all architecture options before committing

## Current Architecture Summary

Based on codebase exploration:

| Component | File | Current Pattern |
|-----------|------|-----------------|
| File I/O | `src/io/reader.rs` | `read_file()` → `Arc<[u8]>` (full file in memory) |
| Chunking | `src/chunking/traits.rs` | `chunk(&str)` → `Vec<Chunk>` (batch processing) |
| Storage | `src/storage/sqlite.rs` | `insert_chunks(Vec<Chunk>)` (batch insert) |
| Search | `src/search/mod.rs` | `hybrid_search()` → `Vec<SearchResult>` |

**Key Traits**: All traits require `Send + Sync` for thread safety.

---

## Architecture Options Comparison

### Option A: Synchronous Iterator Pattern

**Approach**: Extend existing traits with iterator-based streaming methods.

**API Design**:
```rust
pub trait StreamingChunker: Send + Sync {
    fn stream_chunks<R: Read + Send>(
        &self,
        buffer_id: i64,
        source: R,
        metadata: Option<&ChunkMetadata>,
    ) -> Result<Box<dyn ChunkStream>>;
}
```

| Aspect | Assessment |
|--------|------------|
| Complexity | Low - standard Rust patterns |
| Dependencies | None new |
| Binary size impact | Minimal |
| CLI pipeline fit | Excellent |
| Network support | Limited (blocking I/O) |
| Memory control | Easy to bound |

**CLI Usage**:
```bash
cat large_file.txt | rlm-rs load --stdin --name "piped"
rlm-rs search "query" --stream | head -10
tail -f /var/log/app.log | rlm-rs load --stdin --incremental
```

---

### Option B: Async Streams (tokio/futures)

**Approach**: Use `Stream` trait with async/await for non-blocking I/O.

**API Design**:
```rust
#[async_trait]
pub trait AsyncChunker: Send + Sync {
    async fn stream_chunks<R: AsyncRead + Send + Unpin + 'static>(
        &self,
        buffer_id: i64,
        source: R,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Chunk>> + Send>>>;
}
```

| Aspect | Assessment |
|--------|------------|
| Complexity | Medium-High |
| Dependencies | +tokio, +futures, +async-trait |
| Binary size impact | +1-2MB |
| CLI pipeline fit | Good (via block_on) |
| Network support | Excellent (timeouts, cancellation) |
| Memory control | Good |

---

### Option C: Channel-Based Pipeline

**Approach**: Multi-stage pipeline with dedicated threads.

```
[Reader Thread] ──channel──> [Chunker Thread] ──channel──> [Storage Thread]
```

| Aspect | Assessment |
|--------|------------|
| Complexity | High |
| Dependencies | +crossbeam |
| Binary size impact | Minimal |
| CLI pipeline fit | Good |
| Network support | Good |
| Throughput | Highest (parallel stages) |

**Best for**: Batch processing servers, heavy embedding workloads.

---

### Option D: Hybrid Approach (Recommended for Full Requirements)

**Approach**: Sync iterators for file/stdin, async for network (feature-gated).

```rust
pub enum InputSource {
    Sync(Box<dyn Read + Send>),
    #[cfg(feature = "async-network")]
    Async(Box<dyn AsyncRead + Send + Unpin>),
}
```

| Aspect | Assessment |
|--------|------------|
| Complexity | Medium |
| CLI pipeline fit | Excellent |
| Network support | Excellent (when enabled) |
| Flexibility | Maximum |

---

## Recommendation Matrix

| Requirement | Recommended Option |
|-------------|-------------------|
| CLI file/stdin only | **Sync Iterator** |
| CLI + occasional network | **Hybrid** |
| Heavy network/API use | **Async Streams** |
| Batch processing server | **Channel Pipeline** |

**For rlm-rs CLI primary use case**: Start with **Sync Iterator**, extend to **Hybrid** later.

---

## CLI Pipeline Implementation Details

### `cat file.txt | rlm-rs load --stdin`

```
┌──────┐    ┌─────────────────────────────────────────┐
│ cat  │───>│ rlm-rs load --stdin                     │
│      │    │   stdin → BufReader → Chunker → Storage │
└──────┘    └─────────────────────────────────────────┘
```

Key points:
- Detect pipe vs TTY: `!std::io::stdin().is_terminal()` (via `std::io::IsTerminal`, stable since Rust 1.70)
- Use `BufReader` with 64KB buffer
- Batch storage writes (100 chunks per transaction)
- Progress to stderr (doesn't interfere with pipeline)

### `rlm-rs search "query" | head -10`

```
┌─────────────────────┐    ┌──────┐
│ rlm-rs search       │───>│ head │
│   write(line)       │    │      │
│   if EPIPE: break   │    └──────┘
└─────────────────────┘
```

Key points:
- Install `signal(SIGPIPE, SIG_IGN)` on Unix
- Check `write_all()` for `BrokenPipe` error
- Flush after each line

### Signal Handling

```rust
// SIGPIPE: Ignore, let EPIPE propagate
// SIGINT (Ctrl+C): Set shutdown flag, second Ctrl+C force exits
ctrlc::set_handler(|| {
    if SHUTDOWN.swap(true, Ordering::SeqCst) {
        std::process::exit(130);  // Second Ctrl+C
    }
})?;
```

---

## Implementation Plan

### Phase 1: Sync Iterator Foundation
1. Add `StdinReader` to `src/io/stdin.rs`
2. Add `StreamingChunker` trait to `src/chunking/streaming.rs`
3. Implement streaming for `FixedChunker`
4. Add `--stdin` flag to `load` command
5. Add signal handling (SIGPIPE, SIGINT)
6. Add progress reporting to stderr

### Phase 2: Streaming Output
1. Add `--stream` flag to `search` command
2. Line-by-line output mode
3. Handle `head -N` gracefully (EPIPE)

### Phase 3: Storage Batching
1. Add `stream_insert_chunks()` with configurable batch size
2. Transaction batching for efficiency

### Phase 4 (Optional): Network Sources
1. Add `async-network` feature flag
2. Add `--url` option to load command
3. Implement `AsyncChunker` with timeout support

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/io/stdin.rs` | **NEW** - StdinReader implementation |
| `src/io/mod.rs` | Re-export stdin module |
| `src/chunking/streaming.rs` | **NEW** - StreamingChunker trait |
| `src/chunking/fixed.rs` | Add streaming implementation |
| `src/chunking/mod.rs` | Re-export streaming types |
| `src/storage/sqlite.rs` | Add `stream_insert_chunks()` |
| `src/cli/parser.rs` | Add `--stdin`, `--stream` flags |
| `src/cli/commands.rs` | Wire up streaming pipeline |
| `src/cli/signals.rs` | **NEW** - Signal handling |
| `src/cli/progress.rs` | **NEW** - Progress reporting |
| `src/main.rs` | Install signal handlers |

---

## Verification

1. **Unit tests**:
   - Stream chunking with various inputs
   - UTF-8 boundary handling at chunk edges
   - Overlap buffer correctness

2. **Integration tests**:
   - `echo "test" | cargo run -- load --stdin`
   - `cargo run -- search "query" | head -5`
   - Large file streaming without OOM

3. **Memory tests**:
   - Process 1GB file, verify <50MB peak memory
   - `valgrind --tool=massif` profiling

4. **Signal tests**:
   - Ctrl+C graceful shutdown
   - Pipe to `head` without errors
