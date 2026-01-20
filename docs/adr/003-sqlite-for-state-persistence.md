---
title: "SQLite for State Persistence"
description: "Decision to use SQLite as the storage backend for RLM state, chunks, and embeddings"
type: adr
category: storage
tags:
  - sqlite
  - storage
  - persistence
  - database
status: accepted
created: 2025-01-01
updated: 2025-01-01
author: zircote
project: rlm-rs
technologies:
  - sqlite
  - rusqlite
  - fts5
audience:
  - developers
related:
  - 001-adopt-recursive-language-model-pattern
  - 008-hybrid-search-with-rrf
---

# ADR-003: SQLite for State Persistence

## Status

Accepted

## Context

### Background and Problem Statement

The RLM pattern requires persistent storage for:
- Buffer content and metadata
- Chunks with byte offsets and line numbers
- Embedding vectors for semantic search
- Full-text search indexes
- Context state across sessions

The storage solution must support efficient queries, work locally without a server, and integrate well with a CLI tool distribution model.

### Current Limitations

1. **In-memory only**: Would lose state between CLI invocations
2. **File-based JSON**: Inefficient for large datasets, no query capability
3. **External databases**: Require separate installation and configuration

## Decision Drivers

### Primary Decision Drivers

1. **Zero-configuration**: Storage should work out-of-the-box without setup
2. **Embedded operation**: No separate server process required
3. **Query capability**: Need efficient filtering, joining, and full-text search

### Secondary Decision Drivers

1. **ACID compliance**: Data integrity across crashes
2. **Single-file storage**: Easy backup and portability
3. **FTS5 support**: Built-in full-text search for BM25 ranking

## Considered Options

### Option 1: SQLite with rusqlite

**Description**: Use SQLite as an embedded database accessed via rusqlite.

**Technical Characteristics**:
- Single-file database
- ACID compliant
- FTS5 for full-text search
- BLOB storage for embeddings
- No server process

**Advantages**:
- Zero configuration - just a file path
- Excellent query performance for local data
- FTS5 provides BM25 ranking out-of-the-box
- rusqlite is mature and well-maintained
- Single file for easy backup/sync

**Disadvantages**:
- Limited concurrent write performance
- No built-in vector search (must implement)
- File locking can be tricky

**Risk Assessment**:
- **Technical Risk**: Low. SQLite is battle-tested
- **Schedule Risk**: Low. rusqlite API is straightforward
- **Ecosystem Risk**: Low. SQLite is ubiquitous

### Option 2: RocksDB

**Description**: Use RocksDB key-value store for high-performance storage.

**Technical Characteristics**:
- LSM tree architecture
- High write throughput
- Column families for organization

**Advantages**:
- Excellent write performance
- Good compression
- Proven at scale

**Disadvantages**:
- No SQL query capability
- No built-in full-text search
- More complex setup
- Larger dependency footprint

**Disqualifying Factor**: Lack of SQL queries and FTS would require significant additional implementation.

**Risk Assessment**:
- **Technical Risk**: Medium. More complex to use correctly
- **Schedule Risk**: High. Would need custom query layer
- **Ecosystem Risk**: Low. Mature library

### Option 3: PostgreSQL (External)

**Description**: Use PostgreSQL with pgvector for production-grade storage.

**Technical Characteristics**:
- Full SQL support
- pgvector for native vector search
- Requires server process

**Advantages**:
- Native vector similarity search
- Excellent query optimizer
- Scalable

**Disadvantages**:
- Requires separate installation
- Server process management
- Overkill for CLI tool

**Disqualifying Factor**: External server requirement conflicts with zero-configuration CLI design.

**Risk Assessment**:
- **Technical Risk**: Low. Mature database
- **Schedule Risk**: Medium. Setup complexity
- **Ecosystem Risk**: Low. Industry standard

## Decision

Use SQLite as the embedded storage backend via rusqlite.

The implementation will use:
- **rusqlite** for database access with bundled SQLite
- **FTS5** for full-text search with BM25 ranking
- **BLOB columns** for embedding vector storage
- **Foreign keys** for referential integrity
- **Schema migrations** for version upgrades

## Consequences

### Positive

1. **Zero configuration**: Database created automatically on first use
2. **Single-file portability**: `.rlm` file can be copied, backed up, or synced
3. **Built-in FTS5**: BM25 search available without additional dependencies
4. **ACID guarantees**: Data integrity even on crashes
5. **Familiar SQL**: Easy to debug and extend

### Negative

1. **Manual vector search**: Must implement cosine similarity in application code
2. **Write contention**: Concurrent writes require careful handling
3. **No native vector index**: Linear scan for semantic search (mitigated by small dataset sizes)

### Neutral

1. **File locking**: SQLite handles this but requires WAL mode for best concurrency

## Decision Outcome

SQLite provides the right balance of capability and simplicity for a CLI tool. The FTS5 extension enables BM25 search, and embedding vectors stored as BLOBs support semantic search with application-level cosine similarity.

Mitigations:
- Use WAL mode for better concurrency
- Implement efficient batch operations
- Keep result sets small with LIMIT clauses
- Schema versioning for upgrades

## Related Decisions

- [ADR-001: Adopt RLM Pattern](001-adopt-recursive-language-model-pattern.md) - Requires persistent storage
- [ADR-008: Hybrid Search with RRF](008-hybrid-search-with-rrf.md) - Uses FTS5 and embeddings

## Links

- [SQLite](https://www.sqlite.org/) - SQLite documentation
- [rusqlite](https://github.com/rusqlite/rusqlite) - Rust SQLite bindings
- [FTS5](https://www.sqlite.org/fts5.html) - Full-text search extension

## More Information

- **Date:** 2025-01-01
- **Source:** Project inception design decisions
- **Related ADRs:** ADR-001, ADR-008

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| Schema defined with FTS5 | `src/storage/schema.rs` | L88-L108 | compliant |
| Migrations implemented | `src/storage/schema.rs` | L127-L197 | compliant |
| BLOB embedding storage | `src/storage/schema.rs` | L79-L86 | compliant |
| rusqlite dependency | `Cargo.toml` | L30 | compliant |

**Summary:** SQLite storage fully implemented with FTS5, migrations, and embedding support.

**Action Required:** None
