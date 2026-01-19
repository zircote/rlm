# GitHub Copilot Instructions

This document provides context for GitHub Copilot when working with this Rust project.

## Project Context

This is a Rust crate using modern tooling:
- **Rust**: 1.80+ (2024 edition)
- **Build System**: Cargo
- **Linting**: clippy with pedantic and nursery lints
- **Formatting**: rustfmt
- **Testing**: Built-in test framework + proptest
- **Supply Chain Security**: cargo-deny

## Code Generation Guidelines

### Error Handling

Use `Result` types instead of panicking:

```rust
// Good - Returns Result
pub fn parse_value(input: &str) -> Result<i64, ParseError> {
    input.parse().map_err(|e| ParseError::InvalidFormat(e))
}

// Avoid - Panics on failure
pub fn parse_value(input: &str) -> i64 {
    input.parse().unwrap() // Never do this
}
```

Use `thiserror` for custom error types:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("operation failed: {operation}")]
    OperationFailed { operation: String },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

### Type Annotations

Provide explicit return types for public functions:

```rust
// Good - explicit return type
pub fn process_data(items: &[String]) -> Vec<ProcessedItem> {
    // implementation
}

// Avoid for public APIs
pub fn process_data(items: &[String]) -> impl Iterator<Item = ProcessedItem> {
    // implementation
}
```

### Documentation

Use doc comments with examples:

```rust
/// Processes a list of items according to the given configuration.
///
/// # Arguments
///
/// * `items` - The items to process.
/// * `config` - Configuration options for processing.
///
/// # Returns
///
/// A vector of processed items.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if any item is invalid.
///
/// # Examples
///
/// ```rust
/// use {{crate_name}}::{process, Config};
///
/// let items = vec!["a", "b", "c"];
/// let config = Config::default();
/// let result = process(&items, &config)?;
/// # Ok::<(), {{crate_name}}::Error>(())
/// ```
pub fn process(items: &[&str], config: &Config) -> Result<Vec<Item>> {
    // implementation
}
```

### Ownership and Borrowing

Prefer borrowing over ownership when possible:

```rust
// Good - borrows the slice
pub fn sum_values(values: &[i64]) -> i64 {
    values.iter().sum()
}

// Avoid - takes ownership unnecessarily
pub fn sum_values(values: Vec<i64>) -> i64 {
    values.iter().sum()
}
```

Use `Cow` for efficient string handling:

```rust
use std::borrow::Cow;

pub fn normalize_name(name: &str) -> Cow<'_, str> {
    if name.contains(' ') {
        Cow::Owned(name.replace(' ', "_"))
    } else {
        Cow::Borrowed(name)
    }
}
```

### Structs and Enums

Use builder pattern for complex structs:

```rust
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub timeout: Duration,
    pub retries: u32,
    pub verbose: bool,
}

impl Config {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    #[must_use]
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }
}
```

### Testing

Write comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive_numbers() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_divide_by_zero_returns_error() {
        let result = divide(10, 0);
        assert!(matches!(result, Err(Error::DivisionByZero)));
    }
}
```

Use proptest for property-based testing:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn add_is_commutative(a in any::<i64>(), b in any::<i64>()) {
        prop_assert_eq!(add(a, b), add(b, a));
    }
}
```

### Async Code

Use async/await with tokio:

```rust
pub async fn fetch_data(url: &str) -> Result<Data> {
    let response = reqwest::get(url).await?;
    let data = response.json().await?;
    Ok(data)
}
```

## Common Patterns

### Iterator Chains

```rust
let result: Vec<_> = items
    .iter()
    .filter(|item| item.is_valid())
    .map(|item| item.transform())
    .collect();
```

### Option and Result Combinators

```rust
// Option chaining
let value = config
    .get_setting("key")
    .and_then(|s| s.parse().ok())
    .unwrap_or_default();

// Result chaining with ?
fn process() -> Result<Output> {
    let data = load_data()?;
    let parsed = parse_data(&data)?;
    let result = transform(parsed)?;
    Ok(result)
}
```

### Const Functions

Prefer `const fn` for compile-time evaluation:

```rust
#[must_use]
pub const fn new() -> Self {
    Self { value: 0 }
}
```

## File Locations

- Source code: `src/`
- Library entry: `src/lib.rs`
- Binary entry: `src/main.rs`
- Integration tests: `tests/`
- Benchmarks: `benches/` (with criterion)

## Commands

```bash
cargo build           # Build
cargo test            # Run tests
cargo clippy          # Lint
cargo fmt             # Format
cargo doc --open      # Generate and view docs
cargo deny check      # Check supply chain
```
