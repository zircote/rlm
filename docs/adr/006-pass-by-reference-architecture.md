---
title: "Pass-by-Reference Architecture"
description: "Decision to use chunk IDs for retrieval rather than embedding full content in search results"
type: adr
category: architecture
tags:
  - retrieval
  - context-efficiency
  - chunk-ids
  - pass-by-reference
status: accepted
created: 2025-01-15
updated: 2025-01-15
author: zircote
project: rlm-rs
technologies:
  - rust
  - sqlite
audience:
  - developers
  - integrators
related:
  - 001-adopt-recursive-language-model-pattern
  - 003-sqlite-for-state-persistence
  - 008-hybrid-search-with-rrf
---

# ADR-006: Pass-by-Reference Architecture

## Status

Accepted

## Context

### Background and Problem Statement

When integrating rlm-rs with LLM tools like Claude Code, search results need to be communicated efficiently. The question is whether to:
1. Return full chunk content in search results (pass-by-value)
2. Return chunk IDs that can be dereferenced later (pass-by-reference)

This affects context window usage, response latency, and workflow flexibility.

### Current Limitations

1. **Context bloat**: Embedding full content in search results consumes LLM context
2. **Wasted tokens**: Search results may include irrelevant chunks that are never used
3. **Inflexibility**: Cannot defer content retrieval to a later step

## Decision Drivers

### Primary Decision Drivers

1. **Context efficiency**: Minimize LLM context usage until content is actually needed
2. **Selective retrieval**: Allow retrieving only chunks that will be used
3. **Integration flexibility**: Support two-phase workflows (search then fetch)

### Secondary Decision Drivers

1. **Performance**: Smaller search responses are faster
2. **Debugging**: Chunk IDs provide stable references for logging
3. **Caching**: IDs enable caching strategies

## Considered Options

### Option 1: Pass-by-Reference (Chunk IDs)

**Description**: Search returns chunk IDs and metadata; content retrieved separately via `chunk` command.

**Technical Characteristics**:
- Search returns: `{id, score, buffer_name, byte_range, preview}`
- Separate `chunk <id>` command retrieves full content
- Two-phase workflow: search → select → fetch

**Advantages**:
- Minimal context usage in search phase
- LLM can decide which chunks to retrieve
- Smaller, faster search responses
- Stable references for multi-step workflows

**Disadvantages**:
- Requires additional command invocation
- Slight complexity increase for simple use cases

**Risk Assessment**:
- **Technical Risk**: Low. Simple ID lookup
- **Schedule Risk**: Low. Straightforward implementation
- **Ecosystem Risk**: Low. Pattern is well-understood

### Option 2: Pass-by-Value (Full Content)

**Description**: Search returns complete chunk content in results.

**Technical Characteristics**:
- Search returns: `{id, score, content, ...metadata}`
- Single command for search and retrieval
- All content immediately available

**Advantages**:
- Simpler single-step workflow
- No additional commands needed

**Disadvantages**:
- Large responses consume context
- Wasted tokens for unused chunks
- Slower response times
- Cannot scale to large result sets

**Disqualifying Factor**: Context inefficiency makes this unsuitable for LLM integration where context is precious.

**Risk Assessment**:
- **Technical Risk**: Low. Simple to implement
- **Schedule Risk**: Low. Less code
- **Ecosystem Risk**: Medium. Poor scaling

### Option 3: Hybrid with Configurable Expansion

**Description**: Return IDs by default, optionally expand content inline.

**Technical Characteristics**:
- Default: ID-only results
- `--expand` flag includes content
- Best of both approaches

**Advantages**:
- Flexibility for different use cases
- Backwards compatible

**Disadvantages**:
- More complex API
- Users must choose mode

**Risk Assessment**:
- **Technical Risk**: Low. Straightforward flag
- **Schedule Risk**: Low. Minor addition
- **Ecosystem Risk**: Low. Optional feature

## Decision

Adopt pass-by-reference as the primary architecture with IDs for chunk retrieval.

The implementation will provide:
- **`search` command** returning chunk IDs, scores, and previews (truncated content)
- **`chunk` command** for retrieving full content by ID
- **Preview field** with first ~100 characters for quick assessment
- **JSON output** with structured chunk references

## Consequences

### Positive

1. **Context efficiency**: Search results use minimal context; content fetched on-demand
2. **LLM agency**: LLM can intelligently select which chunks to retrieve
3. **Scalable results**: Can return many search hits without context explosion
4. **Stable references**: Chunk IDs persist across sessions for reproducible workflows

### Negative

1. **Two-step workflow**: Simple use cases require search + chunk commands
2. **Learning curve**: Users must understand ID-based retrieval pattern

### Neutral

1. **Preview field**: Provides quick content glimpse without full retrieval

## Decision Outcome

Pass-by-reference enables efficient LLM integration by keeping search responses small and deferring content retrieval to when it's needed. This pattern is essential for Claude Code hooks where context efficiency directly impacts quality.

Mitigations:
- Include preview field in search results for quick assessment
- Clear documentation on two-phase workflow
- Consider future `--expand` flag for simple use cases

## Related Decisions

- [ADR-001: Adopt RLM Pattern](001-adopt-recursive-language-model-pattern.md) - Requires efficient retrieval
- [ADR-003: SQLite Storage](003-sqlite-for-state-persistence.md) - Enables fast ID lookups
- [ADR-008: Hybrid Search](008-hybrid-search-with-rrf.md) - Search produces the IDs

## Links

- [Architecture Documentation](../architecture.md) - Detailed system design

## More Information

- **Date:** 2025-01-15
- **Source:** v1.0.0 release design decisions
- **Related ADRs:** ADR-001, ADR-003, ADR-008

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| Search returns IDs | `src/cli/search.rs` | - | compliant |
| Chunk command implemented | `src/cli/chunk.rs` | - | compliant |
| Preview field in results | `src/storage/search.rs` | - | compliant |
| JSON output structured | `src/cli/` | all | compliant |

**Summary:** Pass-by-reference architecture fully implemented with search and chunk commands.

**Action Required:** None
