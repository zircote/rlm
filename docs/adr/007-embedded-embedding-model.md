---
title: "Embedded Embedding Model"
description: "Decision to embed the embedding model within the binary using fastembed-rs and ONNX runtime"
type: adr
category: technology
tags:
  - embeddings
  - fastembed
  - onnx
  - semantic-search
status: accepted
created: 2025-01-15
updated: 2025-01-20
author: zircote
project: rlm-rs
technologies:
  - fastembed-rs
  - onnx-runtime
  - bge-m3
audience:
  - developers
related:
  - 001-adopt-recursive-language-model-pattern
  - 008-hybrid-search-with-rrf
  - 010-switch-to-bge-m3-model
---

# ADR-007: Embedded Embedding Model

## Status

Accepted

## Context

### Background and Problem Statement

Semantic search requires converting text into embedding vectors that capture meaning. This requires an embedding model. The question is whether to:
1. Call an external API (OpenAI, Cohere, etc.)
2. Run a local model server
3. Embed the model directly in the binary

This decision affects offline capability, privacy, latency, and distribution complexity.

### Current Limitations

1. **API dependencies**: External APIs require network, keys, and incur costs
2. **Server processes**: Local servers add deployment complexity
3. **Privacy concerns**: Sending text to external services may leak sensitive data

## Decision Drivers

### Primary Decision Drivers

1. **Offline capability**: Must work without network connectivity
2. **Privacy**: No data should leave the user's machine
3. **Zero configuration**: Should work out-of-the-box without API keys

### Secondary Decision Drivers

1. **Latency**: Local inference is faster than API calls
2. **Cost**: No per-token charges
3. **Reliability**: No dependency on external service availability

## Considered Options

### Option 1: Embedded Model via fastembed-rs

**Description**: Use fastembed-rs to run ONNX models directly within the Rust binary.

**Technical Characteristics**:
- ONNX Runtime for inference
- Model downloaded on first use
- Lazy loading to minimize cold start
- Thread-safe singleton pattern

**Advantages**:
- Fully offline operation
- No API keys or configuration
- Fast local inference
- Privacy preserved (no data leaves machine)
- Consistent results (no API version drift)

**Disadvantages**:
- Large binary size (ONNX runtime)
- Initial model download required
- CPU-only (no GPU acceleration in default build)
- Memory overhead for model

**Risk Assessment**:
- **Technical Risk**: Low. fastembed-rs is production-ready
- **Schedule Risk**: Low. Drop-in integration
- **Ecosystem Risk**: Low. ONNX is industry standard

### Option 2: External Embedding API

**Description**: Call OpenAI, Cohere, or similar API for embeddings.

**Technical Characteristics**:
- HTTP client for API calls
- API key management
- Rate limiting and retries

**Advantages**:
- Smaller binary size
- Access to latest models
- GPU inference on server side

**Disadvantages**:
- Requires network connectivity
- API key management
- Cost per token
- Privacy concerns
- Latency from network round-trips

**Disqualifying Factor**: Network dependency and privacy concerns conflict with offline-first CLI design.

**Risk Assessment**:
- **Technical Risk**: Low. APIs are well-documented
- **Schedule Risk**: Low. Simple HTTP client
- **Ecosystem Risk**: Medium. API changes, pricing changes

### Option 3: Local Model Server (Ollama, etc.)

**Description**: Require users to run a local embedding server.

**Technical Characteristics**:
- HTTP client to localhost
- External process management
- Model management in server

**Advantages**:
- Offloads inference to dedicated process
- Potentially GPU acceleration
- Model updates independent of rlm-rs

**Disadvantages**:
- Additional installation step
- Process management complexity
- Port conflicts possible

**Disqualifying Factor**: Requiring a separate server conflicts with single-binary distribution goal.

**Risk Assessment**:
- **Technical Risk**: Low. HTTP is simple
- **Schedule Risk**: Medium. Documentation/setup guides needed
- **Ecosystem Risk**: Medium. Server version compatibility

## Decision

Embed the embedding model using fastembed-rs with ONNX runtime.

The implementation will use:
- **fastembed-rs** for model management and inference
- **ONNX Runtime** as the inference backend
- **Lazy loading** to defer model download until first use
- **Thread-safe singleton** for model instance sharing
- **Feature flag** (`fastembed-embeddings`) to make embeddings optional

## Consequences

### Positive

1. **Offline operation**: Works without network after initial model download
2. **Privacy**: No data leaves the user's machine
3. **Zero configuration**: No API keys or server setup required
4. **Consistent**: Same model version produces reproducible results
5. **Fast**: Local inference avoids network latency

### Negative

1. **Binary size**: ONNX runtime adds to binary size
2. **Initial download**: First embedding operation downloads the model (~1.3GB for BGE-M3)
3. **Memory usage**: Model loaded in memory during operation
4. **CPU-only**: No GPU acceleration without custom build

### Neutral

1. **Feature flag**: Embeddings can be disabled for smaller builds

## Decision Outcome

Embedded embeddings via fastembed-rs enable rlm-rs to provide semantic search without external dependencies. The lazy loading pattern minimizes cold start impact for operations that don't need embeddings.

Mitigations:
- Lazy model loading to preserve cold start for non-embedding operations
- Feature flag for builds that don't need semantic search
- Fallback embedder (hash-based) when feature is disabled
- Clear messaging during model download

## Related Decisions

- [ADR-001: Adopt RLM Pattern](001-adopt-recursive-language-model-pattern.md) - Requires semantic embeddings
- [ADR-008: Hybrid Search](008-hybrid-search-with-rrf.md) - Uses embeddings for semantic component
- [ADR-010: Switch to BGE-M3](010-switch-to-bge-m3-model.md) - Current model choice

## Links

- [fastembed-rs](https://github.com/Anush008/fastembed-rs) - Rust embedding library
- [ONNX Runtime](https://onnxruntime.ai/) - Inference engine

## More Information

- **Date:** 2025-01-15
- **Source:** v1.0.0 release design decisions
- **Related ADRs:** ADR-001, ADR-008, ADR-010

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| fastembed-rs integration | `src/embedding/fastembed_impl.rs` | all | compliant |
| Lazy loading singleton | `src/embedding/fastembed_impl.rs` | L14, L59-80 | compliant |
| Feature flag configured | `Cargo.toml` | L17-18 | compliant |
| Fallback embedder available | `src/embedding/fallback.rs` | all | compliant |

**Summary:** Embedded embedding model fully implemented with lazy loading and fallback.

**Action Required:** None
