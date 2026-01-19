# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

{{crate_name}} is a Rust crate built with modern tooling, strict type safety, and zero-cost abstractions.

## Project Structure

```
src/
├── lib.rs           # Library entry point and public API
├── main.rs          # Binary entry point (optional)
├── error.rs         # Error types (if separated)
└── ...              # Additional modules

tests/
└── integration_test.rs  # Integration tests

benches/              # Benchmarks (with criterion)
examples/             # Example programs
```

## Build Commands

This project uses [Cargo](https://doc.rust-lang.org/cargo/) as the build system.

```bash
# Build the project
cargo build

# Build with optimizations
cargo build --release

# Run tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run benchmarks
cargo bench

# Run linting
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Check formatting
cargo fmt -- --check

# Generate documentation
cargo doc --open

# Check supply chain security
cargo deny check

# Run with MIRI (undefined behavior detection)
cargo +nightly miri test

# Run all checks (lint + format + test + doc + deny)
cargo fmt -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && cargo doc --no-deps && cargo deny check
```

## Code Style Requirements

This project uses **clippy** with pedantic and nursery lints, and **rustfmt** for formatting.

### Key Rules

- **Line length**: 100 characters
- **Edition**: 2024
- **MSRV**: 1.80
- **Unsafe code**: Forbidden unless explicitly justified
- **Panics**: Not allowed in library code (`unwrap`, `expect`, `panic!`)

### Error Handling

Always use `Result` types for fallible operations. Never panic in library code:

```rust
// Good - Returns Result
pub fn parse(input: &str) -> Result<Value, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    // parsing logic
    Ok(value)
}

// Bad - Panics
pub fn parse(input: &str) -> Value {
    input.parse().unwrap() // Never do this in library code
}
```

Use `thiserror` for custom error types:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("operation failed")]
    OperationFailed {
        #[source]
        source: std::io::Error,
    },
}
```

### Documentation

All public items must have documentation with examples:

```rust
/// Processes the input data according to the configuration.
///
/// # Arguments
///
/// * `input` - The data to process.
/// * `config` - Processing configuration.
///
/// # Returns
///
/// The processed result.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if the input is malformed.
///
/// # Examples
///
/// ```rust
/// use {{crate_name}}::{process, Config};
///
/// let result = process("data", &Config::default())?;
/// assert!(!result.is_empty());
/// # Ok::<(), {{crate_name}}::Error>(())
/// ```
pub fn process(input: &str, config: &Config) -> Result<Output, Error> {
    // implementation
}
```

### Ownership and Borrowing

Prefer borrowing over ownership:

```rust
// Good - borrows
pub fn process(data: &[u8]) -> Vec<u8> { ... }

// Avoid - takes ownership unnecessarily
pub fn process(data: Vec<u8>) -> Vec<u8> { ... }
```

Use `Cow` for flexible string handling:

```rust
use std::borrow::Cow;

pub fn normalize(s: &str) -> Cow<'_, str> {
    if s.contains(' ') {
        Cow::Owned(s.replace(' ', "_"))
    } else {
        Cow::Borrowed(s)
    }
}
```

### Builder Pattern

Use builder pattern for complex configuration:

```rust
#[derive(Debug, Clone, Default)]
pub struct Config {
    timeout: Duration,
    retries: u32,
}

impl Config {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            retries: 3,
        }
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub const fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
```

## Testing Conventions

- **Unit tests**: Inside `src/*.rs` with `#[cfg(test)]` modules
- **Integration tests**: `tests/` directory
- **Doc tests**: Examples in documentation
- **Property tests**: Use `proptest` for property-based testing

### Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_case() {
        let result = function_under_test(valid_input);
        assert_eq!(result, expected_output);
    }

    #[test]
    fn test_error_case() {
        let result = function_under_test(invalid_input);
        assert!(matches!(result, Err(Error::InvalidInput(_))));
    }
}
```

### Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn property_holds(input in any::<i64>()) {
        prop_assert!(predicate(input));
    }
}
```

## Linting Configuration

Clippy is configured to deny:
- `unwrap_used`, `expect_used`, `panic` - Use Result instead
- `todo`, `unimplemented` - Complete implementation
- `dbg_macro`, `print_stdout`, `print_stderr` - Use proper logging

## Supply Chain Security

This project uses `cargo-deny` to audit dependencies:
- **Advisories**: Deny crates with known vulnerabilities
- **Licenses**: Only allow permissive licenses (MIT, Apache-2.0, BSD)
- **Bans**: Block specific problematic crates
- **Sources**: Only allow crates.io

## Architecture Guidelines

1. **Zero-cost abstractions**: Prefer compile-time over runtime overhead
2. **Explicit over implicit**: No hidden allocations or side effects
3. **Error propagation**: Use `?` operator, avoid `.unwrap()`
4. **Const by default**: Use `const fn` where possible
5. **Minimal dependencies**: Only add what's truly needed
6. **Documentation-driven**: Public API documented with examples

## Performance Considerations

- Use `#[must_use]` for functions returning values that should not be ignored
- Prefer `&str` over `String` in function parameters
- Use `Vec::with_capacity()` when size is known
- Avoid allocations in hot paths
- Profile before optimizing

## CI/CD

The CI pipeline includes:
1. **Format check**: `cargo fmt -- --check`
2. **Lint**: `cargo clippy --all-targets --all-features`
3. **Test**: `cargo test --all-features`
4. **Documentation**: `cargo doc --no-deps`
5. **Supply chain**: `cargo deny check`
6. **MSRV check**: Verify minimum supported Rust version
7. **Coverage**: Generate code coverage reports
