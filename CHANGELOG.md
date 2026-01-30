# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Agent**: Agentic LLM query engine (feature-gated: `agent`)
  - Fan-out/collect pipeline: PrimaryAgent → Search → SubcallAgent×N → SynthesizerAgent
  - `query` command for interactive codebase analysis via LLM
  - Configurable concurrency, batch size, and relevance thresholds
  - Tool-calling loop with internal function dispatch (`get_chunks`, `search`, `grep_chunks`, `get_buffer`, `list_buffers`, `storage_stats`)
  - OpenAI-compatible provider with streaming support
  - Customizable system prompts loaded from disk or environment
- **Agent**: CLI override resolution chain (CLI → Plan → Config → Default)
  - CLI arguments are now `Option` types so clap defaults don't shadow agent plan values
- **Agent**: Cached embedder in `ToolExecutor` via `RefCell`
  - Avoids recreating the ONNX model on every tool call during synthesis
- **Agent**: Bounded grep memory with `MAX_GREP_CHUNKS` cap (5000 chunks)
- **Chunking**: Code-aware chunker for language-specific boundaries
  - Supports Rust, Python, JavaScript, TypeScript, Go, Java, C/C++, Ruby, PHP
  - Splits at function, class, and method boundaries
  - Available via `--chunker code` or `--chunker ast`
- **Search**: HNSW vector index for scalable approximate nearest neighbor search
  - O(log n) search performance
  - Optional feature: enable with `usearch-hnsw` feature flag
- **Search**: Content preview in search results with `--preview` flag
  - Configurable length with `--preview-len` (default: 150 chars)
- **Search**: Batch metadata lookup (`get_chunk_metadata_batch`) eliminates N+1 query pattern
- **Search**: `SearchConfig::with_mode()` builder method for shared search mode parsing
- **Search**: `SearchConfig::with_buffer_id()` for search-layer buffer scoping
- **CLI**: `update-buffer` command to update buffer content with re-chunking
  - Supports `--embed` flag for automatic re-embedding
  - Incremental embedding (only new/changed chunks)
- **CLI**: `dispatch` command for parallel subagent processing
  - Split chunks into batches by size or worker count
  - Filter chunks by search query
- **CLI**: `aggregate` command to combine analyst findings
  - Filter by relevance level using shared `Relevance` enum
  - Group and sort findings
  - Store results in output buffer
- **Core**: `Relevance` enum in `core` module (shared across agent and CLI)
  - Replaces duplicated string-based relevance logic in `commands.rs`
  - Provides `parse()`, `meets_threshold()`, `as_str()`, and `Display`
- **Embedding**: Incremental embedding support
  - Only embeds new or changed chunks
  - Model version tracking for migration detection
- **Embedding**: Model name tracking in `Embedder` trait
- **Output**: NDJSON format support (`--format ndjson`)
- **Testing**: Integration tests for `aggregate`, `dispatch`, and `Relevance` types
- **Documentation**: ADRs for error handling, concurrency model, and feature flags
- **Documentation**: MCP agentic workflow prompts (analyst, orchestrator, synthesizer)
- **MCP**: Model Context Protocol server (feature-gated: `mcp`)
  - `query` tool runs the full agentic pipeline (plan → search → fan-out → synthesis) and returns synthesized results
  - Buffers and chunks exposed as MCP resources at `rlm-rs://{buffer}` and `rlm-rs://{buffer}/{chunk_index}`
  - Resource templates for discoverability
  - Stdio transport for Claude Code integration (`rlm-rs mcp stdio`)
  - Streamable HTTP transport for network access (`rlm-rs mcp sse --host 0.0.0.0 --port 3000`)
  - `spawn_blocking` bridge for `!Send` SQLite storage in async rmcp runtime
- **MCP**: Claude Code orchestrator agent (`.claude/agents/orchestrator.md`)
  - Delegates document analysis to rlm-rs MCP server
  - Parameter guidance for search modes, scaling, and precision tuning

### Changed

- **CLI**: Consolidated 22 flat top-level commands into `buffer`, `chunk`, `context`, and `agent` subcommand groups; old names remain as hidden deprecated aliases
- **Agent**: Default models changed from `gpt-4o` to `gpt-5.2-2025-12-11` (primary, synthesizer) and `gpt-5-mini-2025-08-07` (subcall)
- **Agent**: Added adaptive scaling profiles for dataset-size-aware batch and concurrency tuning
- **Agent**: `fan_out` accepts `Arc<[LoadedChunk]>` to avoid unnecessary clone
- **Agent**: `search_chunks` and `load_chunks` converted to associated functions
  (removed `#[allow(clippy::unused_self)]`)
- **Core**: Consolidated UTF-8 and timestamp utilities in io module
  - `find_char_boundary` and `current_timestamp` now shared across modules
- **Core**: Improved token estimation with `estimate_tokens_accurate()` method
- **Error**: Dedicated `Embedding` error variant in `StorageError`
- **Embedding**: Removed unnecessary unsafe `Send`/`Sync` impls from `FallbackEmbedder`

### Removed

- **Agent**: Dead `ToolSet::subcall_tools()` method (subcall agents have no tools by design)
- **Agent**: Dead `single_shot()` convenience wrapper in `agentic_loop`
- **Agent**: Dead `assistant_message()` helper in `message`
- **Agent**: Unused `AgentError` variants: `RateLimited`, `RetriesExhausted`, `Timeout`
- **Agent**: Duplicate stream assignment in `OpenAiProvider::build_request`

### Fixed

- **Search**: USearch segfault via PR #704 move semantics fix
- **CLI**: UTF-8 safe string truncation using `find_char_boundary` (prevents panic on multi-byte characters)
- **CLI**: NDJSON format now emits newline-delimited single-line JSON (was identical to pretty-printed JSON)
- **CLI**: `init`, `reset`, and `buffer delete` commands now respect `--format json` flag
- **Agent**: Unused error binding in subcall truncation handler
- **CLI**: Redundant clone in `cmd_update_buffer` buffer construction
- **Agent**: `get_buffer` tool schema now requires at least one property (`minProperties: 1`)
- **Agent**: Search pipeline now falls back across modes (hybrid → bm25 → semantic) when the planner's chosen mode returns zero results
- **Agent**: Omit `temperature` parameter for models that only support the default value
- **Docs**: Unresolved intra-doc links in `Chunk` methods

### Security

- **Agent**: XML content tags replace markdown code fences in prompt assembly
  - Chunk content wrapped in `<content>` tags within `<chunk>` elements
  - Query wrapped in `<query>` tags, findings in `<findings>` tags
  - Harder to escape via embedded triple-backtick content
- **Agent**: Untrusted-data handling instructions in subcall and synthesizer system prompts
  - Agents instructed to never interpret embedded directives as commands
  - Output fidelity requirements prevent prompt injection via document content
- **Agent**: Regex pattern validation before compilation in `grep_chunks`
  - `MAX_REGEX_LEN` (500 bytes) prevents oversized patterns
  - DFA size limit (1 MB) via `RegexBuilder` prevents ReDoS attacks
- **Agent**: Tool argument size limits in executor
  - `MAX_CHUNK_IDS` (200), `MAX_SEARCH_TOP_K` (500), `MAX_CONTEXT_LINES` (20)
  - Raw tool arguments capped at 100 KB before dispatch
- **Agent**: Query length validation (`MAX_QUERY_LEN` = 10,000 bytes) in orchestrator
- **Agent**: Finding size limits in subcall response parsing
  - `MAX_FINDINGS_PER_BATCH` (200), `MAX_FINDING_TEXT_LEN` (5,000 bytes), `MAX_FOLLOW_UPS` (10)
  - `sanitize_findings()` truncates oversized responses after parsing

### Dependencies

- Add `rmcp` 0.14 for Model Context Protocol server support
- Add `schemars` 1.0 for MCP tool JSON Schema generation
- Add `axum` 0.8 for streamable HTTP transport
- Add `tokio-util` 0.7 for cancellation token support
- Bump `actions/github-script` from 7 to 8 ([#7])
- Bump `criterion` from 0.5.1 to 0.8.1 ([#9])
- Bump `rusqlite` from 0.33.0 to 0.38.0 ([#8])
- Bump `actions/checkout` from 4 to 6 ([#6])
- Bump `taiki-e/install-action` in the github-actions group ([#5])

## [1.2.3] - 2026-01-20

### Fixed

- **CI**: Allow `multiple_crate_versions` lint (fastembed transitive deps)
- **CI**: Add ISC, BSD, MPL-2.0, CDLA-Permissive-2.0 to allowed licenses
- **CI**: Ignore unmaintained `paste` advisory (fastembed transitive dep)
- **CI**: Skip openssl ban check for fastembed transitive deps

## [1.2.2] - 2026-01-20

### Fixed

- **CLI**: Handle broken pipe gracefully when output is piped to commands like `jq` or `head`

## [1.2.1] - 2026-01-20

### Fixed

- **Build**: Enable `fastembed-embeddings` feature by default (BGE-M3 now works out of the box)

## [1.2.0] - 2026-01-20

### Added

- **Search**: Search results now include `index` (document position) and `buffer_id` fields for temporal ordering
- **Documentation**: Architecture Decision Records (ADRs) documenting 10 key architectural decisions from project history

### Changed

- **Embedding**: Switch from all-MiniLM-L6-v2 to BGE-M3 embedding model
  - Dimensions increased from 384 to 1024 for richer semantic representation
  - Token context increased from ~512 to 8192 for full chunk coverage
  - **Breaking**: Existing embeddings must be regenerated (schema migration v3 clears old embeddings)
- **Build**: Bump MSRV to 1.88

### Fixed

- **Search**: Escape FTS5 special characters in search queries to prevent syntax errors
- **Chunking**: Validate UTF-8 boundaries in semantic chunker search window to prevent panics on multi-byte characters

## [1.1.2] - 2026-01-19

### Changed

- **Chunking**: Reduced default chunk size from 240,000 to 3,000 characters for better semantic search granularity
- **Chunking**: Reduced max chunk size from 250,000 to 50,000 characters

## [1.1.1] - 2026-01-19

### Fixed

- **Search**: BM25 scores now display in scientific notation for small values (e.g., `1.60e-6` instead of `0.0000`)
- **Search**: FTS queries use OR semantics for multi-word searches (more forgiving matching)
- **Embedding**: Auto-embedding during load now outputs proper JSON when `--format json` is used

## [1.1.0] - 2026-01-19

### Added

- **Search**: Hybrid semantic + BM25 search with Reciprocal Rank Fusion (RRF)
- **Search**: `search` command with `--mode` option (`hybrid`, `semantic`, `bm25`)
- **Embedding**: Auto-embedding during `load` command (embeddings generated automatically)
- **Chunks**: `chunk get` command for pass-by-reference retrieval
- **Chunks**: `chunk list` command to list chunks for a buffer
- **Chunks**: `chunk embed` command to generate/regenerate embeddings
- **Chunks**: `chunk status` command to show embedding status

### Changed

- **Load**: Embeddings are now generated automatically during load (no separate embed step needed)

## [1.0.0] - 2026-01-19

### Added

- **Core**: Initial release with semantic search and pass-by-reference architecture
- **Chunking**: Fixed, semantic, and parallel chunking strategies
- **Storage**: SQLite persistence for buffers, chunks, and variables
- **Search**: Regex search with `grep` command
- **I/O**: Memory-mapped file handling for large documents
- **CLI**: JSON output format support for all commands

## [0.2.0] - 2026-01-19

### Added

- **CI/CD**: Release workflow to auto-update Homebrew tap

## [0.1.0] - 2026-01-19

### Added

- Initial implementation of RLM-RS CLI
- Buffer management (load, list, show, delete, peek)
- Chunking with configurable strategies
- Variable storage (context and global)
- Export functionality

[Unreleased]: https://github.com/zircote/rlm-rs/compare/v1.2.3...HEAD
[1.2.3]: https://github.com/zircote/rlm-rs/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/zircote/rlm-rs/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/zircote/rlm-rs/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/zircote/rlm-rs/compare/v1.1.2...v1.2.0
[1.1.2]: https://github.com/zircote/rlm-rs/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/zircote/rlm-rs/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/zircote/rlm-rs/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/zircote/rlm-rs/compare/v0.2.0...v1.0.0
[0.2.0]: https://github.com/zircote/rlm-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/zircote/rlm-rs/releases/tag/v0.1.0
[#5]: https://github.com/zircote/rlm-rs/pull/5
[#6]: https://github.com/zircote/rlm-rs/pull/6
[#7]: https://github.com/zircote/rlm-rs/pull/7
[#8]: https://github.com/zircote/rlm-rs/pull/8
[#9]: https://github.com/zircote/rlm-rs/pull/9
