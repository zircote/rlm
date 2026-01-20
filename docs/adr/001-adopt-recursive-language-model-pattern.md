---
title: "Adopt Recursive Language Model (RLM) Pattern"
description: "Foundation architectural decision to implement the RLM pattern from arXiv:2512.24601 for efficient LLM context management"
type: adr
category: architecture
tags:
  - rlm
  - context-management
  - llm
  - foundational
status: accepted
created: 2025-01-01
updated: 2025-01-01
author: zircote
project: rlm-rs
technologies:
  - rust
  - sqlite
  - embeddings
audience:
  - developers
  - architects
related:
  - 003-sqlite-for-state-persistence
  - 006-pass-by-reference-architecture
---

# ADR-001: Adopt Recursive Language Model (RLM) Pattern

## Status

Accepted

## Context

### Background and Problem Statement

Large Language Models (LLMs) have finite context windows that limit the amount of information they can process in a single interaction. When working with large codebases, documents, or long-running conversations, this limitation becomes a significant constraint. Users need a way to efficiently manage and retrieve relevant context for LLM interactions without manually curating what fits in the context window.

The research paper arXiv:2512.24601 introduces the Recursive Language Model (RLM) pattern, which provides a systematic approach to context management through recursive summarization and intelligent retrieval.

### Current Limitations

1. **Fixed context windows**: LLMs cannot process arbitrarily large inputs, forcing users to manually select relevant portions
2. **Lost context**: In long conversations or large documents, important earlier context gets dropped
3. **Inefficient retrieval**: Without semantic understanding, keyword search often returns irrelevant results
4. **No state persistence**: Each LLM interaction starts fresh without memory of previous sessions

## Decision Drivers

### Primary Decision Drivers

1. **Context efficiency**: Enable processing of arbitrarily large documents within LLM context limits through intelligent chunking and retrieval
2. **Semantic relevance**: Retrieve contextually relevant information rather than just keyword matches
3. **Research foundation**: Build on peer-reviewed research (arXiv:2512.24601) rather than ad-hoc solutions

### Secondary Decision Drivers

1. **Extensibility**: Pattern should support multiple chunking strategies and embedding models
2. **Performance**: Minimize latency in context retrieval operations
3. **Simplicity**: Keep the core abstraction simple enough for CLI usage

## Considered Options

### Option 1: Recursive Language Model (RLM) Pattern

**Description**: Implement the RLM pattern with recursive summarization, semantic chunking, and hybrid search for context retrieval.

**Technical Characteristics**:
- Sliding window context management
- Semantic embeddings for similarity search
- Recursive summarization for compression
- State persistence across sessions

**Advantages**:
- Research-backed approach with theoretical foundation
- Handles arbitrarily large inputs
- Maintains semantic coherence in retrieved context
- Supports incremental context building

**Disadvantages**:
- Complexity in implementing recursive summarization
- Requires embedding model infrastructure
- Learning curve for users unfamiliar with the pattern

**Risk Assessment**:
- **Technical Risk**: Medium. Novel pattern requires careful implementation
- **Schedule Risk**: Medium. Research translation to production code takes time
- **Ecosystem Risk**: Low. Uses standard ML/NLP components

### Option 2: Simple Truncation with Keywords

**Description**: Truncate documents to fit context window with keyword-based retrieval.

**Technical Characteristics**:
- Fixed-size chunking
- BM25 or TF-IDF keyword search
- No semantic understanding

**Advantages**:
- Simple to implement
- No ML dependencies
- Fast retrieval

**Disadvantages**:
- Loses semantic context
- Poor handling of synonyms and related concepts
- No intelligent summarization

**Disqualifying Factor**: Cannot maintain semantic coherence across large documents, defeating the purpose of intelligent context management.

**Risk Assessment**:
- **Technical Risk**: Low. Well-understood approach
- **Schedule Risk**: Low. Quick to implement
- **Ecosystem Risk**: Low. No dependencies

### Option 3: External RAG Service

**Description**: Integrate with external Retrieval-Augmented Generation services.

**Technical Characteristics**:
- API-based retrieval
- Cloud-hosted embeddings and search
- Managed infrastructure

**Advantages**:
- No local ML infrastructure needed
- Scalable
- Managed updates

**Disadvantages**:
- Network dependency
- Privacy concerns with sending data to external services
- Cost at scale
- Vendor lock-in

**Disqualifying Factor**: Privacy concerns and network dependency conflict with CLI-first, local-first design goals.

**Risk Assessment**:
- **Technical Risk**: Low. Mature services available
- **Schedule Risk**: Low. Quick integration
- **Ecosystem Risk**: High. Vendor dependency

## Decision

Adopt the Recursive Language Model (RLM) pattern as described in arXiv:2512.24601 as the foundational architecture for context management.

The implementation will use:
- **Semantic chunking** for intelligent document segmentation
- **Embedding vectors** for semantic similarity search
- **SQLite storage** for persistent state and chunk management
- **Hybrid search** combining semantic and keyword approaches

## Consequences

### Positive

1. **Unlimited input size**: Users can load arbitrarily large documents; the system handles chunking and retrieval automatically
2. **Semantic understanding**: Retrieved context is semantically relevant, not just keyword matches
3. **Session persistence**: Context and summaries persist across CLI invocations
4. **Research foundation**: Implementation backed by peer-reviewed research provides confidence in approach

### Negative

1. **Complexity**: More complex than simple truncation, requiring embedding infrastructure
2. **Initial latency**: First-time embedding generation adds startup cost
3. **Storage overhead**: Embeddings and chunks require disk space

### Neutral

1. **Learning curve**: Users must understand chunking and retrieval concepts, but CLI abstracts most complexity

## Decision Outcome

The RLM pattern provides the theoretical and practical foundation for rlm-rs. It enables the core value proposition: efficient context management for LLM interactions with large documents and codebases.

Mitigations:
- Lazy model loading to reduce cold start impact
- Efficient SQLite schema for fast retrieval
- Clear CLI interface abstracting implementation details

## Related Decisions

- [ADR-003: SQLite for State Persistence](003-sqlite-for-state-persistence.md) - Storage backend for RLM state
- [ADR-006: Pass-by-Reference Architecture](006-pass-by-reference-architecture.md) - Chunk retrieval mechanism

## Links

- [arXiv:2512.24601](https://arxiv.org/abs/2512.24601) - Original RLM research paper
- [RLM-Inspired Design](../rlm-inspired-design.md) - Detailed implementation notes

## More Information

- **Date:** 2025-01-01
- **Source:** arXiv:2512.24601 research paper
- **Related ADRs:** ADR-003, ADR-006, ADR-007, ADR-008

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| RLM pattern implemented in core library | `src/lib.rs` | L1-L50 | compliant |
| Chunking strategies available | `src/chunking/` | all | compliant |
| Embedding infrastructure present | `src/embedding/` | all | compliant |
| SQLite persistence implemented | `src/storage/` | all | compliant |

**Summary:** The RLM pattern has been fully implemented as the foundational architecture.

**Action Required:** None
