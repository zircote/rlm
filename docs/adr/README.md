# Architecture Decision Records

This directory contains Architecture Decision Records (ADRs) for rlm-rs.

## Index

| ID | Title | Status | Date |
|----|-------|--------|------|
| [001](001-adopt-recursive-language-model-pattern.md) | Adopt Recursive Language Model (RLM) Pattern | accepted | 2025-01-01 |
| [002](002-use-rust-as-implementation-language.md) | Use Rust as Implementation Language | accepted | 2025-01-01 |
| [003](003-sqlite-for-state-persistence.md) | SQLite for State Persistence | accepted | 2025-01-01 |
| [004](004-multiple-chunking-strategies.md) | Multiple Chunking Strategies | accepted | 2025-01-01 |
| [005](005-cli-first-interface-design.md) | CLI-First Interface Design | accepted | 2025-01-01 |
| [006](006-pass-by-reference-architecture.md) | Pass-by-Reference Architecture | accepted | 2025-01-15 |
| [007](007-embedded-embedding-model.md) | Embedded Embedding Model | accepted | 2025-01-15 |
| [008](008-hybrid-search-with-rrf.md) | Hybrid Search with Reciprocal Rank Fusion | accepted | 2025-01-17 |
| [009](009-reduced-default-chunk-size.md) | Reduced Default Chunk Size | accepted | 2025-01-18 |
| [010](010-switch-to-bge-m3-model.md) | Switch to BGE-M3 Embedding Model | accepted | 2025-01-20 |

## Status Legend

- **proposed** - Under discussion, not yet decided
- **accepted** - Decision made and approved
- **deprecated** - No longer relevant but kept for historical reference
- **superseded** - Replaced by a newer ADR
- **rejected** - Considered but not accepted

## Creating New ADRs

Use the `/adr-new` command to create a new ADR with the Structured MADR template.

## References

- [MADR](https://adr.github.io/madr/) - Markdown Architectural Decision Records
- [ADR GitHub Organization](https://adr.github.io/)
