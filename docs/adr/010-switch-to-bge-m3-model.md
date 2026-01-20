---
title: "Switch to BGE-M3 Embedding Model"
description: "Decision to switch from all-MiniLM-L6-v2 to BGE-M3 for improved context window and search quality"
type: adr
category: technology
tags:
  - embeddings
  - bge-m3
  - fastembed
  - schema-migration
status: accepted
created: 2025-01-20
updated: 2025-01-20
author: zircote
project: rlm-rs
technologies:
  - fastembed-rs
  - bge-m3
  - onnx-runtime
audience:
  - developers
related:
  - 007-embedded-embedding-model
  - 008-hybrid-search-with-rrf
---

# ADR-010: Switch to BGE-M3 Embedding Model

## Status

Accepted

## Context

### Background and Problem Statement

The original embedding model choice (all-MiniLM-L6-v2) was made for its small size and fast inference. However, production usage revealed limitations:
- 384 dimensions provide lower semantic resolution
- ~512 token context often truncates larger chunks
- Multilingual support is limited

BGE-M3 offers significant improvements at the cost of larger model size.

### Current Limitations

1. **Dimension mismatch**: 384 dimensions limit semantic expressiveness
2. **Token truncation**: ~512 token limit truncates chunks near the 2000-byte default size
3. **Multilingual gaps**: MiniLM has limited non-English support

## Decision Drivers

### Primary Decision Drivers

1. **Context coverage**: BGE-M3's 8192 token limit covers full chunks without truncation
2. **Embedding quality**: 1024 dimensions provide richer semantic representation
3. **Consistency**: Full chunk content is embedded, not truncated

### Secondary Decision Drivers

1. **Multilingual support**: BGE-M3 handles non-English content better
2. **Model maturity**: BGE-M3 is well-established in the embedding community
3. **fastembed support**: Both models supported by fastembed-rs

## Considered Options

### Option 1: Switch to BGE-M3

**Description**: Replace all-MiniLM-L6-v2 with BGE-M3 embedding model.

**Technical Characteristics**:
- 1024 dimensions (vs 384)
- 8192 token context (vs ~512)
- ~1.3GB model size (vs ~90MB)
- Stronger multilingual support

**Advantages**:
- Full chunk coverage without truncation
- Higher semantic resolution
- Better multilingual embeddings
- More accurate semantic search

**Disadvantages**:
- Larger model download (~1.3GB vs ~90MB)
- Slightly slower inference
- Breaking change: requires schema migration
- Existing embeddings must be regenerated

**Risk Assessment**:
- **Technical Risk**: Low. fastembed-rs supports BGE-M3 well
- **Schedule Risk**: Low. Simple model swap
- **Ecosystem Risk**: Low. Well-established model

### Option 2: Keep all-MiniLM-L6-v2

**Description**: Maintain current model.

**Technical Characteristics**:
- 384 dimensions
- ~512 token context
- ~90MB model size

**Advantages**:
- Smaller model download
- Faster inference
- No migration needed

**Disadvantages**:
- Continued truncation issues
- Lower semantic quality
- Limited multilingual support

**Disqualifying Factor**: Token truncation undermines semantic search quality for typical chunk sizes.

**Risk Assessment**:
- **Technical Risk**: None. No change
- **Schedule Risk**: None. No change
- **Ecosystem Risk**: Low. Status quo

### Option 3: External Embedding API

**Description**: Switch to OpenAI or similar API embeddings.

**Technical Characteristics**:
- API-based embedding generation
- Higher quality models available
- Requires network and API key

**Advantages**:
- Access to latest models
- No local model storage

**Disadvantages**:
- Network dependency
- Privacy concerns
- API costs
- Conflicts with offline-first design

**Disqualifying Factor**: Violates offline-first and privacy principles established in ADR-007.

**Risk Assessment**:
- **Technical Risk**: Low. APIs are simple
- **Schedule Risk**: Low. Easy integration
- **Ecosystem Risk**: High. API dependency

## Decision

Switch from all-MiniLM-L6-v2 to BGE-M3 as the default embedding model.

The implementation will:
- Change `EmbeddingModel::AllMiniLML6V2` to `EmbeddingModel::BGEM3`
- Update `DEFAULT_DIMENSIONS` from 384 to 1024
- Add schema migration (v2→v3) to clear incompatible embeddings
- Keep model download silent (existing behavior)

## Consequences

### Positive

1. **Full chunk coverage**: 8192 tokens handles any reasonable chunk size
2. **Better search quality**: 1024 dimensions capture more semantic nuance
3. **Multilingual improvement**: Better handling of non-English content
4. **Future-proof**: More headroom for chunk size increases

### Negative

1. **Breaking change**: Existing embeddings incompatible (different dimensions)
2. **Larger download**: ~1.3GB model vs ~90MB
3. **Slower inference**: Larger model has higher compute cost
4. **Migration required**: Users must regenerate embeddings after upgrade

### Neutral

1. **Model download timing**: Same lazy loading pattern as before

## Decision Outcome

BGE-M3 provides a significant quality improvement for semantic search. The migration clears existing embeddings, requiring users to re-embed their content, but this is a one-time cost for lasting quality improvements.

Mitigations:
- Schema migration (v2→v3) automatically clears old embeddings
- Clear error messages guide users to re-embed
- Document migration in CHANGELOG
- Lazy loading preserves cold start for non-embedding operations

## Related Decisions

- [ADR-007: Embedded Embedding Model](007-embedded-embedding-model.md) - Embedding infrastructure
- [ADR-008: Hybrid Search](008-hybrid-search-with-rrf.md) - Semantic search uses embeddings

## Links

- [BGE-M3 Paper](https://arxiv.org/abs/2402.03216) - Model research paper
- [fastembed-rs](https://github.com/Anush008/fastembed-rs) - Rust embedding library
- [BAAI/bge-m3](https://huggingface.co/BAAI/bge-m3) - Hugging Face model card

## More Information

- **Date:** 2025-01-20
- **Source:** Production usage feedback and quality analysis
- **Related ADRs:** ADR-007, ADR-008

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| BGE-M3 model configured | `src/embedding/fastembed_impl.rs` | L66 | compliant |
| DEFAULT_DIMENSIONS = 1024 | `src/embedding/mod.rs` | L27 | compliant |
| Schema version bumped to 3 | `src/storage/schema.rs` | L6 | compliant |
| Migration clears embeddings | `src/storage/schema.rs` | L179-183 | compliant |

**Summary:** BGE-M3 model switch fully implemented with schema migration.

**Action Required:** None
