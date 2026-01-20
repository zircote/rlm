---
title: "Hybrid Search with Reciprocal Rank Fusion"
description: "Decision to combine semantic and BM25 search using RRF for improved retrieval quality"
type: adr
category: search
tags:
  - search
  - hybrid
  - rrf
  - bm25
  - semantic
status: accepted
created: 2025-01-17
updated: 2025-01-17
author: zircote
project: rlm-rs
technologies:
  - fts5
  - embeddings
  - rrf
audience:
  - developers
related:
  - 003-sqlite-for-state-persistence
  - 007-embedded-embedding-model
---

# ADR-008: Hybrid Search with Reciprocal Rank Fusion

## Status

Accepted

## Context

### Background and Problem Statement

Effective context retrieval requires finding chunks that are both semantically similar to the query and contain relevant keywords. Two primary search approaches exist:
1. **Semantic search**: Uses embedding similarity to find conceptually related content
2. **Keyword search (BM25)**: Uses term frequency to find exact/near-exact matches

Each has strengths and weaknesses. The question is how to combine them effectively.

### Current Limitations

1. **Semantic-only**: Misses exact keyword matches; poor for specific identifiers
2. **Keyword-only**: Misses conceptual similarity; poor for paraphrased queries
3. **Simple averaging**: Doesn't account for different score scales between methods

## Decision Drivers

### Primary Decision Drivers

1. **Retrieval quality**: Must find both conceptually and lexically relevant chunks
2. **Robustness**: Should handle queries that favor either semantic or keyword matching
3. **Simplicity**: Fusion method should be easy to implement and tune

### Secondary Decision Drivers

1. **Performance**: Fusion should not significantly slow search
2. **Explainability**: Users should understand why results rank as they do
3. **Tunability**: Allow adjusting semantic vs keyword balance

## Considered Options

### Option 1: Reciprocal Rank Fusion (RRF)

**Description**: Combine results using RRF formula: `score = sum(1 / (k + rank))` across result lists.

**Technical Characteristics**:
- Rank-based fusion (not score-based)
- Hyperparameter k (typically 60) controls rank sensitivity
- Simple to implement
- Well-studied in information retrieval

**Advantages**:
- Doesn't require score normalization
- Robust to different score distributions
- Proven effective in research literature
- Simple formula, easy to implement

**Disadvantages**:
- Loses absolute score information
- k parameter requires tuning for optimal results

**Risk Assessment**:
- **Technical Risk**: Low. Well-understood algorithm
- **Schedule Risk**: Low. Simple implementation
- **Ecosystem Risk**: Low. No dependencies

### Option 2: Linear Score Combination

**Description**: Normalize scores and combine with weighted average.

**Technical Characteristics**:
- Min-max or z-score normalization
- Weighted sum: `α * semantic + (1-α) * bm25`
- Score-based fusion

**Advantages**:
- Preserves score magnitude information
- Simple weight tuning

**Disadvantages**:
- Requires careful score normalization
- Sensitive to score distribution differences
- Normalization can be unstable with few results

**Risk Assessment**:
- **Technical Risk**: Medium. Normalization is tricky
- **Schedule Risk**: Low. Simple formula
- **Ecosystem Risk**: Low. No dependencies

### Option 3: Learning-to-Rank

**Description**: Train a model to combine features into optimal ranking.

**Technical Characteristics**:
- Feature extraction from both search methods
- Trained ranking model
- Requires labeled training data

**Advantages**:
- Can learn optimal combination
- Handles complex interactions

**Disadvantages**:
- Requires training data
- Model adds complexity
- Overkill for current use case

**Disqualifying Factor**: Requires labeled training data that doesn't exist for this use case.

**Risk Assessment**:
- **Technical Risk**: High. ML training complexity
- **Schedule Risk**: High. Data collection, training pipeline
- **Ecosystem Risk**: Medium. Model dependencies

## Decision

Use Reciprocal Rank Fusion (RRF) to combine semantic and BM25 search results.

The implementation will use:
- **SQLite FTS5** for BM25 keyword search
- **Cosine similarity** for semantic search on embeddings
- **RRF formula** with k=60 for score fusion
- **Configurable limit** for result count

Formula: `rrf_score(d) = Σ 1/(k + rank_i(d))` where i iterates over semantic and BM25 rankings.

## Consequences

### Positive

1. **Improved recall**: Finds chunks that either method alone would miss
2. **Robustness**: Works well for both conceptual and exact-match queries
3. **No normalization needed**: RRF uses ranks, not scores
4. **Simplicity**: Easy to understand and debug

### Negative

1. **Score interpretation**: RRF scores don't directly represent relevance
2. **Two searches required**: Must run both semantic and keyword search
3. **Parameter tuning**: k=60 may not be optimal for all cases

### Neutral

1. **Performance overhead**: Running two searches adds latency but is acceptable

## Decision Outcome

Hybrid search with RRF significantly improves retrieval quality by combining the strengths of semantic and keyword search. The implementation in v1.1.0 provides a robust search foundation for LLM context retrieval.

Mitigations:
- Default k=60 based on IR literature
- Future: expose k as configurable parameter if needed
- Preview field helps users assess result relevance

## Related Decisions

- [ADR-003: SQLite Storage](003-sqlite-for-state-persistence.md) - FTS5 provides BM25
- [ADR-007: Embedded Embeddings](007-embedded-embedding-model.md) - Provides semantic vectors

## Links

- [Reciprocal Rank Fusion](https://plg.uwaterloo.ca/~gvcormac/cormacksigir09-rrf.pdf) - Original RRF paper
- [SQLite FTS5](https://www.sqlite.org/fts5.html) - BM25 implementation

## More Information

- **Date:** 2025-01-17
- **Source:** v1.1.0 release design decisions
- **Related ADRs:** ADR-003, ADR-007

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| RRF implementation | `src/storage/search.rs` | - | compliant |
| Semantic search function | `src/storage/search.rs` | - | compliant |
| BM25 via FTS5 | `src/storage/schema.rs` | L88-108 | compliant |
| Search CLI command | `src/cli/search.rs` | - | compliant |

**Summary:** Hybrid search with RRF fully implemented combining semantic and BM25 search.

**Action Required:** None
