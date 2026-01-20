---
title: "Reduced Default Chunk Size"
description: "Decision to reduce default chunk size from 4000 to 2000 bytes for improved search precision"
type: adr
category: configuration
tags:
  - chunking
  - search
  - configuration
  - defaults
status: accepted
created: 2025-01-18
updated: 2025-01-18
author: zircote
project: rlm-rs
technologies:
  - chunking
audience:
  - developers
  - users
related:
  - 004-multiple-chunking-strategies
  - 008-hybrid-search-with-rrf
---

# ADR-009: Reduced Default Chunk Size

## Status

Accepted

## Context

### Background and Problem Statement

The default chunk size affects:
- Search precision: Smaller chunks = more precise retrieval
- Context efficiency: Smaller chunks = less wasted context
- Embedding quality: Embedding models have optimal input sizes
- Chunking overhead: More chunks = more storage and processing

The original default of 4000 bytes was chosen conservatively. After production usage, it became clear that smaller chunks would improve the system.

### Current Limitations

1. **Large chunks dilute relevance**: A 4000-byte chunk may contain one relevant sentence and much irrelevant content
2. **Context waste**: Retrieved chunks often contain more text than needed
3. **Embedding quality**: Longer text may exceed optimal embedding model context

## Decision Drivers

### Primary Decision Drivers

1. **Search precision**: Smaller chunks increase retrieval precision
2. **Context efficiency**: Smaller chunks reduce wasted LLM context tokens
3. **Embedding model alignment**: 2000 bytes aligns better with typical model token limits

### Secondary Decision Drivers

1. **User feedback**: Reports of imprecise search results
2. **Empirical testing**: Better search quality observed with smaller chunks
3. **Backward compatibility**: Existing databases should still work (they keep their chunk size)

## Considered Options

### Option 1: Reduce to 2000 bytes

**Description**: Change default from 4000 to 2000 bytes.

**Technical Characteristics**:
- ~500-600 tokens per chunk (model-dependent)
- Fits comfortably in embedding model context
- Good balance of precision and coherence

**Advantages**:
- More precise search results
- Less wasted context in retrieved chunks
- Better embedding quality
- Still large enough for coherent units

**Disadvantages**:
- More chunks per document
- Slightly more storage overhead
- May break some semantic units

**Risk Assessment**:
- **Technical Risk**: Low. Simple constant change
- **Schedule Risk**: Low. Minimal code change
- **Ecosystem Risk**: Low. Backwards compatible

### Option 2: Keep 4000 bytes

**Description**: Maintain status quo.

**Technical Characteristics**:
- ~1000-1200 tokens per chunk
- Current behavior preserved

**Advantages**:
- No change required
- Preserves larger semantic units

**Disadvantages**:
- Continues precision issues
- Wastes context on irrelevant content

**Risk Assessment**:
- **Technical Risk**: None. No change
- **Schedule Risk**: None. No change
- **Ecosystem Risk**: Low. Status quo

### Option 3: Reduce to 1000 bytes

**Description**: More aggressive reduction to 1000 bytes.

**Technical Characteristics**:
- ~250-300 tokens per chunk
- Very fine-grained retrieval

**Advantages**:
- Maximum precision
- Minimal context waste

**Disadvantages**:
- May fragment semantic units
- Many more chunks to manage
- Higher storage overhead
- May lose broader context

**Risk Assessment**:
- **Technical Risk**: Medium. May be too aggressive
- **Schedule Risk**: Low. Simple change
- **Ecosystem Risk**: Low. Backwards compatible

## Decision

Reduce the default chunk size from 4000 to 2000 bytes.

The implementation will:
- Change `DEFAULT_CHUNK_SIZE` constant from 4000 to 2000
- Existing databases retain their original chunk sizes
- Users can override with `--chunk-size` flag

## Consequences

### Positive

1. **Improved precision**: Search results contain more focused, relevant content
2. **Better context efficiency**: Less wasted tokens in LLM context
3. **Embedding alignment**: 2000 bytes fits well within embedding model optimal ranges
4. **User satisfaction**: Addresses feedback about imprecise results

### Negative

1. **More chunks**: Documents produce ~2x more chunks than before
2. **Re-chunking needed**: Users wanting new default must reload documents
3. **Storage increase**: Slightly more metadata per document

### Neutral

1. **Backward compatibility**: Existing databases continue to work

## Decision Outcome

The 2000-byte default provides a better balance of precision and coherence based on production usage feedback. Users who prefer larger chunks can still use `--chunk-size 4000`.

Mitigations:
- Document the change in CHANGELOG
- Provide migration guidance for users wanting to re-chunk
- Keep `--chunk-size` flag for customization

## Related Decisions

- [ADR-004: Multiple Chunking Strategies](004-multiple-chunking-strategies.md) - Chunking framework
- [ADR-008: Hybrid Search](008-hybrid-search-with-rrf.md) - Chunk size affects search quality

## Links

- [CHANGELOG v1.1.2](../../CHANGELOG.md) - Release notes documenting change

## More Information

- **Date:** 2025-01-18
- **Source:** v1.1.2 release based on user feedback
- **Related ADRs:** ADR-004, ADR-008

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| DEFAULT_CHUNK_SIZE = 2000 | `src/chunking/mod.rs` | - | compliant |
| --chunk-size flag available | `src/main.rs` | - | compliant |
| CHANGELOG documents change | `CHANGELOG.md` | v1.1.2 | compliant |

**Summary:** Default chunk size reduced to 2000 bytes with CLI override available.

**Action Required:** None
