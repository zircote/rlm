# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.4] - 2026-02-08

### Added

- **Chunking**: Code-aware chunker for language-specific boundaries
  - Supports Rust, Python, JavaScript, TypeScript, Go, Java, C/C++, Ruby, PHP
  - Splits at function, class, and method boundaries
  - Available via `--chunker code` or `--chunker ast`
- **Search**: HNSW vector index for scalable approximate nearest neighbor search
  - O(log n) search performance
  - Optional feature: enable with `usearch-hnsw` feature flag
- **Search**: Content preview in search results with `--preview` flag
  - Configurable length with `--preview-len` (default: 150 chars)
- **CLI**: `update-buffer` command to update buffer content with re-chunking
  - Supports `--embed` flag for automatic re-embedding
  - Incremental embedding (only new/changed chunks)
- **CLI**: `dispatch` command for parallel subagent processing
  - Split chunks into batches by size or worker count
  - Filter chunks by search query
- **CLI**: `aggregate` command to combine analyst findings
  - Filter by relevance level
  - Group and sort findings
  - Store results in output buffer
- **Embedding**: Incremental embedding support
  - Only embeds new or changed chunks
  - Model version tracking for migration detection
- **Embedding**: Model name tracking in `Embedder` trait
- **Output**: NDJSON format support (`--format ndjson`)
- **CI**: Crates.io publish workflow with pre-publish validation
- **CI**: Dependabot auto-merge workflow
- **Documentation**: ADRs for error handling, concurrency model, and feature flags
- **Documentation**: MCP agentic workflow prompts (analyst, orchestrator, synthesizer)

### Changed

- **Core**: Consolidated UTF-8 and timestamp utilities in io module
  - `find_char_boundary` and `current_timestamp` now shared across modules
- **Core**: Improved token estimation with `estimate_tokens_accurate()` method
- **Error**: Dedicated `Embedding` error variant in `StorageError`
- **Embedding**: Removed unnecessary unsafe `Send`/`Sync` impls from `FallbackEmbedder`

### Fixed

- **Search**: Resolve usearch segfault via move semantics fix
- **Documentation**: Fix unresolved intra-doc links in Chunk methods

### Security

- Update `bytes` to 1.11.1 to resolve RUSTSEC-2026-0007

### Dependencies

- Bump `bytes` from 1.10.1 to 1.11.1
- Bump `clap` from 4.5.54 to 4.5.56 ([#11])
- Bump the github-actions group with 2 updates ([#12])
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

[Unreleased]: https://github.com/zircote/rlm-rs/compare/v1.2.4...HEAD
[1.2.4]: https://github.com/zircote/rlm-rs/compare/v1.2.3...v1.2.4
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
