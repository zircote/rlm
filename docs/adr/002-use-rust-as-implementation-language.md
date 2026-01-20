---
title: "Use Rust as Implementation Language"
description: "Decision to implement rlm-rs in Rust for performance, safety, and single-binary distribution"
type: adr
category: technology
tags:
  - rust
  - language
  - performance
  - safety
status: accepted
created: 2025-01-01
updated: 2025-01-01
author: zircote
project: rlm-rs
technologies:
  - rust
  - cargo
audience:
  - developers
related:
  - 001-adopt-recursive-language-model-pattern
---

# ADR-002: Use Rust as Implementation Language

## Status

Accepted

## Context

### Background and Problem Statement

Implementing the RLM pattern requires a language that can efficiently handle:
- Large text processing and chunking operations
- Embedding vector computations
- SQLite database operations
- CLI interface with good user experience

The implementation language choice affects performance, distribution complexity, and long-term maintainability.

### Current Limitations

1. **Python ML tools**: Require runtime installation and dependency management
2. **JavaScript/Node**: V8 overhead for compute-intensive embedding operations
3. **Go**: Limited ML ecosystem, would require CGO for embedding models

## Decision Drivers

### Primary Decision Drivers

1. **Single binary distribution**: Users should be able to install via a single executable without runtime dependencies
2. **Performance**: Text processing and vector operations should be efficient
3. **Memory safety**: No segfaults or memory leaks in production use

### Secondary Decision Drivers

1. **Type safety**: Strong typing catches errors at compile time
2. **Ecosystem**: Cargo provides excellent dependency management
3. **Cross-platform**: Should compile for Linux, macOS, and Windows

## Considered Options

### Option 1: Rust

**Description**: Implement in Rust using cargo for builds and distribution.

**Technical Characteristics**:
- Zero-cost abstractions
- No garbage collector
- Single static binary output
- Strong type system with ownership model

**Advantages**:
- Single binary distribution (no runtime required)
- Memory safety without garbage collection
- Excellent performance for text processing
- Strong ecosystem for CLI (clap) and database (rusqlite)
- fastembed-rs provides native embedding support

**Disadvantages**:
- Steeper learning curve
- Longer compile times
- Smaller talent pool than Python/JS

**Risk Assessment**:
- **Technical Risk**: Low. Mature language with stable tooling
- **Schedule Risk**: Medium. Rust requires more upfront design
- **Ecosystem Risk**: Low. Key dependencies (rusqlite, fastembed) are mature

### Option 2: Python

**Description**: Implement in Python with PyInstaller or similar for distribution.

**Technical Characteristics**:
- Dynamic typing
- Rich ML ecosystem (numpy, sentence-transformers)
- Requires Python runtime or bundled interpreter

**Advantages**:
- Fastest development velocity
- Best ML library ecosystem
- Large developer community

**Disadvantages**:
- Distribution complexity (virtualenv, pip, version conflicts)
- Performance overhead for text processing
- Memory management less predictable

**Disqualifying Factor**: Distribution complexity conflicts with goal of simple single-binary CLI tool.

**Risk Assessment**:
- **Technical Risk**: Low. Very mature ecosystem
- **Schedule Risk**: Low. Fast development
- **Ecosystem Risk**: Medium. Dependency conflicts common

### Option 3: Go

**Description**: Implement in Go for simple distribution and good performance.

**Technical Characteristics**:
- Static binary compilation
- Garbage collected
- Simple language design

**Advantages**:
- Single binary distribution
- Fast compilation
- Good CLI tooling (cobra)

**Disadvantages**:
- Limited ML/embedding ecosystem
- Would require CGO for ONNX runtime
- Less expressive type system

**Disqualifying Factor**: Embedding model integration would require complex CGO bindings.

**Risk Assessment**:
- **Technical Risk**: Medium. CGO complexity for ML
- **Schedule Risk**: Medium. ML integration work
- **Ecosystem Risk**: High. Limited embedding options

## Decision

Use Rust as the implementation language for rlm-rs.

The implementation will use:
- **Cargo** for build system and dependency management
- **clap** for CLI argument parsing
- **rusqlite** for SQLite database access
- **fastembed-rs** for embedding generation
- **thiserror** for error handling

## Consequences

### Positive

1. **Zero-dependency distribution**: Users install a single binary with no runtime requirements
2. **Predictable performance**: No GC pauses, efficient memory usage
3. **Compile-time safety**: Ownership system prevents memory bugs and data races
4. **Cross-platform builds**: cargo handles cross-compilation well

### Negative

1. **Development velocity**: Rust requires more upfront design than Python
2. **Compile times**: Full rebuilds take longer than interpreted languages
3. **Contributor barrier**: Fewer developers familiar with Rust

### Neutral

1. **Binary size**: Statically linked binaries are larger but self-contained

## Decision Outcome

Rust enables the core distribution goal: a single binary CLI tool that users can install and run without dependency management. The performance characteristics are well-suited for text processing and embedding operations.

Mitigations:
- Use incremental compilation during development
- Provide clear documentation for contributors
- Leverage Rust's excellent documentation and error messages

## Related Decisions

- [ADR-001: Adopt RLM Pattern](001-adopt-recursive-language-model-pattern.md) - Architecture this implements

## Links

- [Rust Programming Language](https://www.rust-lang.org/) - Official Rust website
- [fastembed-rs](https://github.com/Anush008/fastembed-rs) - Rust embedding library

## More Information

- **Date:** 2025-01-01
- **Source:** Project inception design decisions
- **Related ADRs:** ADR-001

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| Rust edition 2024 configured | `Cargo.toml` | L3 | compliant |
| MSRV 1.88 specified | `Cargo.toml` | L7 | compliant |
| Strict clippy lints enabled | `Cargo.toml` | L89-L120 | compliant |
| No unsafe code blocks | all | - | compliant |

**Summary:** Project fully implemented in Rust with strict safety configuration.

**Action Required:** None
