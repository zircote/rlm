---
title: "Multiple Chunking Strategies"
description: "Decision to support pluggable chunking strategies for different content types and use cases"
type: adr
category: architecture
tags:
  - chunking
  - text-processing
  - extensibility
status: accepted
created: 2025-01-01
updated: 2025-01-01
author: zircote
project: rlm-rs
technologies:
  - rust
  - text-processing
audience:
  - developers
related:
  - 001-adopt-recursive-language-model-pattern
  - 009-reduced-default-chunk-size
---

# ADR-004: Multiple Chunking Strategies

## Status

Accepted

## Context

### Background and Problem Statement

The RLM pattern requires breaking large documents into chunks for:
- Embedding generation (models have token limits)
- Semantic search (smaller chunks = more precise retrieval)
- Context assembly (select relevant chunks for LLM input)

Different content types benefit from different chunking approaches:
- Code benefits from syntax-aware chunking
- Prose benefits from paragraph/sentence boundaries
- Structured data may need fixed-size chunks

### Current Limitations

1. **Single strategy**: One-size-fits-all chunking loses semantic coherence
2. **Token limits**: Embedding models have maximum input sizes
3. **Search precision**: Large chunks reduce retrieval precision

## Decision Drivers

### Primary Decision Drivers

1. **Content-aware chunking**: Preserve semantic units (paragraphs, functions, etc.)
2. **Token budget compliance**: Chunks must fit within embedding model limits
3. **Extensibility**: Users should be able to add custom strategies

### Secondary Decision Drivers

1. **Overlap support**: Allow overlapping chunks to preserve context at boundaries
2. **Metadata preservation**: Track byte offsets and line numbers for each chunk
3. **Performance**: Chunking should be fast even for large files

## Considered Options

### Option 1: Pluggable Strategy Pattern

**Description**: Define a `ChunkingStrategy` trait with multiple implementations users can select.

**Technical Characteristics**:
- Trait-based abstraction
- Runtime strategy selection
- Consistent chunk metadata across strategies

**Advantages**:
- Users choose appropriate strategy per content type
- Easy to add new strategies
- Consistent interface for all strategies
- Strategies can be combined or chained

**Disadvantages**:
- More code complexity than single implementation
- Users must understand options

**Risk Assessment**:
- **Technical Risk**: Low. Strategy pattern is well-understood
- **Schedule Risk**: Low. Core strategies straightforward
- **Ecosystem Risk**: Low. No external dependencies

### Option 2: Single Adaptive Strategy

**Description**: One smart chunker that auto-detects content type and adapts.

**Technical Characteristics**:
- Content type detection
- Heuristic-based splitting
- Single code path

**Advantages**:
- Simpler API (no strategy selection)
- "Just works" for most cases

**Disadvantages**:
- Heuristics may fail for edge cases
- Hard to tune for specific needs
- Complex implementation

**Disqualifying Factor**: Cannot handle all content types well with one algorithm.

**Risk Assessment**:
- **Technical Risk**: High. Heuristics are fragile
- **Schedule Risk**: Medium. Detection logic complex
- **Ecosystem Risk**: Low. Self-contained

### Option 3: External Chunking Library

**Description**: Use an existing text chunking library.

**Technical Characteristics**:
- External dependency
- Pre-built strategies
- May have language detection

**Advantages**:
- Faster initial development
- Battle-tested implementations

**Disadvantages**:
- Limited Rust options available
- May not match exact requirements
- Additional dependency

**Disqualifying Factor**: No suitable Rust library with required features existed at project inception.

**Risk Assessment**:
- **Technical Risk**: Medium. Dependency quality varies
- **Schedule Risk**: Low. If library exists
- **Ecosystem Risk**: Medium. Dependency maintenance

## Decision

Implement a pluggable chunking system with multiple strategies via a `ChunkingStrategy` trait.

The implementation will provide:
- **FixedSize**: Simple byte/character count splitting
- **Paragraph**: Split on blank lines (markdown/prose)
- **Sentence**: Split on sentence boundaries
- **Sliding Window**: Overlapping chunks for context preservation
- **Recursive**: Tree-sitter or regex-based for code

## Consequences

### Positive

1. **Content-appropriate chunking**: Users select strategy matching their content
2. **Extensibility**: New strategies can be added without changing core code
3. **Consistent metadata**: All strategies produce chunks with offsets and line numbers
4. **Testability**: Each strategy can be tested in isolation

### Negative

1. **User choice required**: Users must understand which strategy to use
2. **More code**: Multiple implementations to maintain
3. **Potential confusion**: Too many options can overwhelm

### Neutral

1. **Default strategy**: Providing a sensible default mitigates choice paralysis

## Decision Outcome

The strategy pattern enables rlm-rs to handle diverse content types effectively. The `--strategy` CLI flag lets users select the appropriate chunker, with paragraph chunking as a sensible default.

Mitigations:
- Good documentation explaining when to use each strategy
- Sensible defaults for common cases
- Clear error messages when chunks exceed limits

## Related Decisions

- [ADR-001: Adopt RLM Pattern](001-adopt-recursive-language-model-pattern.md) - Chunking is core to RLM
- [ADR-009: Reduced Default Chunk Size](009-reduced-default-chunk-size.md) - Default size tuning

## Links

- [text-splitter crate](https://crates.io/crates/text-splitter) - Rust text splitting (evaluated)

## More Information

- **Date:** 2025-01-01
- **Source:** Project inception design decisions
- **Related ADRs:** ADR-001, ADR-009

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| ChunkingStrategy trait defined | `src/chunking/strategy.rs` | - | compliant |
| Multiple strategies implemented | `src/chunking/` | all | compliant |
| CLI --strategy flag | `src/main.rs` | - | compliant |
| Chunk metadata tracked | `src/chunking/chunk.rs` | - | compliant |

**Summary:** Pluggable chunking system fully implemented with multiple strategies.

**Action Required:** None
