# Contributing to RLM-RS

Thank you for your interest in contributing to `rlm-rs`! This document provides guidelines and instructions for contributing.

## Code of Conduct

Please be respectful and constructive in all interactions. We welcome contributors of all backgrounds and experience levels.

## Getting Started

### Prerequisites

- **Rust 1.85+** (2024 edition)
- **cargo-deny** for supply chain security checks

```bash
# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install cargo-deny
cargo install cargo-deny
```

### Setting Up the Development Environment

```bash
# Clone the repository
git clone https://github.com/zircote/rlm-rs.git
cd rlm-rs

# Build the project
cargo build

# Run tests
cargo test

# Run the full CI check
make ci
```

## Development Workflow

### Branch Strategy

- `main` - Stable release branch
- Feature branches - `feature/<description>`
- Bug fixes - `fix/<description>`

### Making Changes

1. **Fork the repository** and create a branch from `main`
2. **Write your code** following the style guidelines below
3. **Add tests** for any new functionality
4. **Run the full CI check** before submitting:

```bash
make ci
```

This runs:
- `cargo fmt -- --check` (formatting)
- `cargo clippy --all-targets --all-features` (linting)
- `cargo test` (tests)
- `cargo doc --no-deps` (documentation)
- `cargo deny check` (supply chain)

### Commit Messages

Use clear, descriptive commit messages:

```
<type>: <short description>

<optional longer description>
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

Examples:
```
feat: add parallel chunking strategy

fix: handle UTF-8 boundary in fixed chunker

docs: update API reference for Storage trait
```

## Code Style Guidelines

### General Rules

- **Line length**: 100 characters maximum
- **Edition**: Rust 2024
- **Unsafe code**: Forbidden unless explicitly justified with comments
- **Panics**: Not allowed in library code (`unwrap`, `expect`, `panic!`)

### Formatting

Code must pass `cargo fmt`:

```bash
# Check formatting
cargo fmt -- --check

# Auto-format
cargo fmt
```

### Linting

Code must pass strict clippy lints:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

The project uses pedantic and nursery lints. Key rules enforced:

| Lint | Rule |
|------|------|
| `unwrap_used` | Denied - use `Result` instead |
| `expect_used` | Denied - use `Result` instead |
| `panic` | Denied - handle errors gracefully |
| `todo` | Denied - complete implementation |
| `dbg_macro` | Denied - remove debug macros |
| `print_stdout` | Denied - use proper logging |

### Error Handling

Always use `Result` types for fallible operations:

```rust
// Good
pub fn parse(input: &str) -> Result<Value, ParseError> {
    if input.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    Ok(value)
}

// Bad - panics
pub fn parse(input: &str) -> Value {
    input.parse().unwrap() // Never do this
}
```

Use `thiserror` for custom error types:

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
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

All public items must have documentation:

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
/// use rlm_rs::{process, Config};
///
/// let result = process("data", &Config::default())?;
/// assert!(!result.is_empty());
/// # Ok::<(), rlm_rs::Error>(())
/// ```
pub fn process(input: &str, config: &Config) -> Result<Output, Error> {
    // implementation
}
```

### Ownership and Borrowing

Prefer borrowing over ownership when possible:

```rust
// Good - borrows
pub fn process(data: &[u8]) -> Vec<u8> { ... }

// Avoid - takes ownership unnecessarily
pub fn process(data: Vec<u8>) -> Vec<u8> { ... }
```

### Const Functions

Use `const fn` where possible:

```rust
#[must_use]
pub const fn new() -> Self {
    Self {
        size: DEFAULT_SIZE,
        overlap: DEFAULT_OVERLAP,
    }
}
```

## Testing

### Test Organization

- **Unit tests**: Inside `src/*.rs` with `#[cfg(test)]` modules
- **Integration tests**: `tests/` directory
- **Doc tests**: Examples in documentation

### Writing Tests

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

For complex invariants, use `proptest`:

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn chunk_boundaries_valid(content in ".{1,1000}") {
        let chunker = FixedChunker::with_size(100);
        let chunks = chunker.chunk(1, &content, None).unwrap();
        for chunk in chunks {
            prop_assert!(chunk.byte_range.end <= content.len());
        }
    }
}
```

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name

# Run tests for a specific module
cargo test chunking::
```

## Adding New Features

### Adding a New Chunking Strategy

1. Create `src/chunking/my_strategy.rs`:

```rust
use crate::chunking::traits::{Chunker, ChunkMetadata};
use crate::core::Chunk;
use crate::error::Result;

pub struct MyChunker {
    chunk_size: usize,
}

impl MyChunker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            chunk_size: super::DEFAULT_CHUNK_SIZE,
        }
    }
}

impl Chunker for MyChunker {
    fn chunk(
        &self,
        buffer_id: i64,
        text: &str,
        metadata: Option<&ChunkMetadata>,
    ) -> Result<Vec<Chunk>> {
        // Implementation
    }

    fn name(&self) -> &'static str {
        "my-strategy"
    }

    fn description(&self) -> &'static str {
        "Description of my chunking strategy"
    }
}
```

2. Export in `src/chunking/mod.rs`:

```rust
pub mod my_strategy;
pub use my_strategy::MyChunker;
```

3. Add to `create_chunker` factory function.

4. Add tests and documentation.

### Adding a New CLI Command

1. Add variant to `Commands` enum in `src/cli/parser.rs`:

```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    // ...existing commands...

    /// Description of my command.
    MyCommand {
        /// Argument description.
        #[arg(short, long)]
        my_arg: String,
    },
}
```

2. Implement handler in `src/cli/commands.rs`:

```rust
Commands::MyCommand { my_arg } => {
    // Implementation
    Ok("Output".to_string())
}
```

3. Add tests and update CLI reference documentation.

## Pull Request Process

1. **Ensure all checks pass**:
   ```bash
   make ci
   ```

2. **Update documentation** if needed:
   - README.md for user-facing changes
   - docs/ for detailed documentation
   - Code comments for internal changes

3. **Create a pull request** with:
   - Clear title describing the change
   - Description of what and why
   - Link to related issues (if any)

4. **Address review feedback** promptly

### PR Checklist

- [ ] Code follows style guidelines
- [ ] All tests pass (`cargo test`)
- [ ] Clippy passes (`cargo clippy`)
- [ ] Format is correct (`cargo fmt`)
- [ ] Documentation updated (if needed)
- [ ] No new warnings introduced

## Reporting Issues

### Bug Reports

Include:
- Rust version (`rustc --version`)
- OS and version
- Steps to reproduce
- Expected vs actual behavior
- Relevant logs or error messages

### Feature Requests

Include:
- Use case description
- Proposed solution (if any)
- Alternatives considered

## Project Structure

```
src/
├── lib.rs           # Library entry point
├── main.rs          # CLI entry point
├── error.rs         # Error types
├── core/            # Core domain types
│   ├── buffer.rs    # Buffer type
│   ├── chunk.rs     # Chunk type
│   └── context.rs   # Context/variables
├── chunking/        # Chunking strategies
│   ├── traits.rs    # Chunker trait
│   ├── fixed.rs     # Fixed chunker
│   ├── semantic.rs  # Semantic chunker
│   └── parallel.rs  # Parallel chunker
├── storage/         # Persistence
│   ├── traits.rs    # Storage trait
│   └── sqlite.rs    # SQLite backend
├── io/              # File I/O
│   ├── reader.rs    # File reading
│   └── unicode.rs   # Unicode utilities
└── cli/             # CLI layer
    ├── parser.rs    # Argument parsing
    ├── commands.rs  # Command handlers
    └── output.rs    # Output formatting

tests/
└── integration_test.rs

docs/
├── architecture.md  # Internal architecture
├── cli-reference.md # CLI documentation
└── api.md           # Library API reference
```

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

## Questions?

- Open an issue for questions
- Check existing issues and documentation first

Thank you for contributing!
