---
title: "CLI-First Interface Design"
description: "Decision to build rlm-rs as a CLI tool with composable Unix-style commands"
type: adr
category: interface
tags:
  - cli
  - unix
  - composability
  - user-experience
status: accepted
created: 2025-01-01
updated: 2025-01-01
author: zircote
project: rlm-rs
technologies:
  - rust
  - clap
audience:
  - developers
  - users
related:
  - 002-use-rust-as-implementation-language
---

# ADR-005: CLI-First Interface Design

## Status

Accepted

## Context

### Background and Problem Statement

rlm-rs needs a user interface for:
- Loading and managing buffers
- Chunking content with various strategies
- Searching for relevant context
- Retrieving chunks by ID
- Managing persistent state

The interface design affects usability, scriptability, and integration with other tools (especially LLM clients like Claude Code).

### Current Limitations

1. **GUI complexity**: Building GUIs adds significant development overhead
2. **Web services**: Require running a server, complicate distribution
3. **Library-only**: Requires users to write code for basic operations

## Decision Drivers

### Primary Decision Drivers

1. **Unix philosophy**: Small, composable tools that do one thing well
2. **Scriptability**: Commands should be easily automated and piped
3. **Developer workflow**: Target audience uses terminals daily

### Secondary Decision Drivers

1. **Low overhead**: No server process, just invoke and exit
2. **JSON output**: Machine-readable output for integration
3. **Discoverability**: Built-in help and documentation

## Considered Options

### Option 1: CLI with Subcommands

**Description**: Build a CLI tool with subcommands (like git) using clap.

**Technical Characteristics**:
- Subcommand pattern: `rlm-rs <command> [args]`
- Consistent flag patterns across commands
- JSON output mode for scripting
- Exit codes for error handling

**Advantages**:
- Familiar pattern for developers
- Composable with shell pipelines
- No runtime dependencies for users
- Easy to script and automate
- Works well with Claude Code hooks

**Disadvantages**:
- No interactive exploration (without TUI)
- Learning curve for command flags
- Verbose for complex operations

**Risk Assessment**:
- **Technical Risk**: Low. clap is mature
- **Schedule Risk**: Low. Well-understood pattern
- **Ecosystem Risk**: Low. Terminal is ubiquitous

### Option 2: REST API Service

**Description**: Run a local HTTP server exposing REST endpoints.

**Technical Characteristics**:
- Long-running process
- HTTP endpoints for operations
- OpenAPI documentation

**Advantages**:
- Language-agnostic client integration
- Stateful operations easier
- Can serve web UI

**Disadvantages**:
- Must manage server lifecycle
- Port conflicts
- More complex deployment

**Disqualifying Factor**: Running a server conflicts with simple CLI tool goal.

**Risk Assessment**:
- **Technical Risk**: Low. HTTP is well-understood
- **Schedule Risk**: Medium. More infrastructure
- **Ecosystem Risk**: Low. HTTP is standard

### Option 3: Interactive TUI

**Description**: Build a terminal user interface with ratatui.

**Technical Characteristics**:
- Full-screen terminal application
- Keyboard navigation
- Real-time display

**Advantages**:
- Rich exploration experience
- Visual feedback
- Interactive workflows

**Disadvantages**:
- Cannot be scripted
- More complex implementation
- Not composable with pipes

**Disqualifying Factor**: Not scriptable, cannot integrate with Claude Code hooks.

**Risk Assessment**:
- **Technical Risk**: Medium. TUI is complex
- **Schedule Risk**: High. Significant UI work
- **Ecosystem Risk**: Low. Terminal support is good

## Decision

Build rlm-rs as a CLI tool with subcommands following Unix philosophy.

The implementation will use:
- **clap** for argument parsing with derive macros
- **Subcommand pattern** for logical grouping
- **JSON output** via `--json` flag for scripting
- **Consistent flags** across commands (e.g., `--db` for database path)

## Consequences

### Positive

1. **Scriptable**: Easy integration with shell scripts and Claude Code hooks
2. **Composable**: Output can be piped to other tools
3. **Familiar**: Developers understand CLI patterns
4. **Portable**: Works in any terminal environment

### Negative

1. **No visual exploration**: Must know commands to use effectively
2. **Verbose commands**: Complex operations require long command lines
3. **No state between commands**: Each invocation is independent (mitigated by SQLite)

### Neutral

1. **JSON output mode**: Adds complexity but essential for scripting

## Decision Outcome

The CLI-first design enables rlm-rs to integrate seamlessly with developer workflows and LLM tools like Claude Code. The `--json` output mode makes it easy to parse results programmatically.

Mitigations:
- Comprehensive `--help` for discoverability
- Sensible defaults to reduce required flags
- Shell completion scripts for convenience
- Examples in documentation

## Related Decisions

- [ADR-002: Use Rust](002-use-rust-as-implementation-language.md) - Enables single-binary CLI

## Links

- [clap](https://docs.rs/clap/) - Rust CLI argument parser
- [Unix Philosophy](https://en.wikipedia.org/wiki/Unix_philosophy) - Design principles

## More Information

- **Date:** 2025-01-01
- **Source:** Project inception design decisions
- **Related ADRs:** ADR-002

## Audit

### 2025-01-20

**Status:** Compliant

**Findings:**

| Finding | Files | Lines | Assessment |
|---------|-------|-------|------------|
| clap derive macros used | `src/main.rs` | - | compliant |
| Subcommand pattern implemented | `src/cli/` | all | compliant |
| JSON output mode available | `src/cli/` | - | compliant |
| Exit codes for errors | `src/main.rs` | - | compliant |

**Summary:** CLI-first design fully implemented with subcommands and JSON output.

**Action Required:** None
