# {{crate_name}}

<!-- Badges -->
[![GitHub Template](https://img.shields.io/badge/template-zircote%2Frust--template-blue?logo=github)](https://github.com/zircote/rust-template)
[![CI](https://github.com/zircote/{{crate_name}}/actions/workflows/ci.yml/badge.svg)](https://github.com/zircote/{{crate_name}}/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/{{crate_name}}.svg?logo=rust&logoColor=white)](https://crates.io/crates/{{crate_name}})
[![Documentation](https://docs.rs/{{crate_name}}/badge.svg)](https://docs.rs/{{crate_name}})
[![Rust Version](https://img.shields.io/badge/rust-1.80%2B-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Clippy](https://img.shields.io/badge/linting-clippy-orange?logo=rust&logoColor=white)](https://github.com/rust-lang/rust-clippy)
[![cargo-deny](https://img.shields.io/badge/security-cargo--deny-blue?logo=rust&logoColor=white)](https://github.com/EmbarkStudios/cargo-deny)
[![Security: gitleaks](https://img.shields.io/badge/security-gitleaks-blue?logo=git&logoColor=white)](https://github.com/gitleaks/gitleaks)
[![Dependabot](https://img.shields.io/badge/dependabot-enabled-025e8c?logo=dependabot)](https://docs.github.com/en/code-security/dependabot)

A Rust crate description.

## Features

- Feature 1
- Feature 2
- Feature 3

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
{{crate_name}} = "0.1"
```

Or use cargo add:

```bash
cargo add {{crate_name}}
```

## Quick Start

```rust
use {{crate_name}}::{add, divide, Config};

fn main() -> Result<(), {{crate_name}}::Error> {
    // Basic arithmetic
    let sum = add(2, 3);
    println!("2 + 3 = {}", sum);

    // Safe division with error handling
    let quotient = divide(10, 2)?;
    println!("10 / 2 = {}", quotient);

    // Using configuration builder
    let config = Config::new()
        .with_verbose(true)
        .with_max_retries(5);

    Ok(())
}
```

## API Overview

### Functions

| Function | Description |
|----------|-------------|
| `add(a, b)` | Adds two numbers |
| `divide(a, b)` | Divides with error handling |

### Types

| Type | Description |
|------|-------------|
| `Config` | Configuration with builder pattern |
| `Error` | Error type for operations |
| `Result<T>` | Type alias for `Result<T, Error>` |

## Development

### Prerequisites

- Rust 1.80+ (2024 edition)
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) for supply chain security

### Setup

```bash
# Clone the repository
git clone https://github.com/zircote/{{crate_name}}.git
cd {{crate_name}}

# Build
cargo build

# Run tests
cargo test

# Run linting
cargo clippy --all-targets --all-features

# Format code
cargo fmt

# Check supply chain security
cargo deny check

# Generate documentation
cargo doc --open
```

### Project Structure

```
src/
├── lib.rs           # Library entry point
├── main.rs          # Binary entry point
└── ...              # Additional modules

tests/
└── integration_test.rs

Cargo.toml           # Project manifest
clippy.toml          # Clippy configuration
rustfmt.toml         # Formatter configuration
deny.toml            # cargo-deny configuration
CLAUDE.md            # AI assistant instructions
```

### Code Quality

This project maintains high code quality standards:

- **Linting**: clippy with pedantic and nursery lints
- **Formatting**: rustfmt with custom configuration
- **Testing**: Unit tests, integration tests, and property-based tests
- **Documentation**: All public APIs documented with examples
- **Supply Chain**: cargo-deny for dependency auditing
- **CI/CD**: GitHub Actions for automated testing

### Running Checks

```bash
# Run all checks
cargo fmt -- --check && \
cargo clippy --all-targets --all-features -- -D warnings && \
cargo test && \
cargo doc --no-deps && \
cargo deny check

# Run with MIRI for undefined behavior detection
cargo +nightly miri test
```

## MSRV Policy

The Minimum Supported Rust Version (MSRV) is **1.80**. Increasing the MSRV is considered a minor breaking change.

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run the test suite (`cargo test`)
5. Run linting (`cargo clippy --all-targets --all-features`)
6. Format code (`cargo fmt`)
7. Commit your changes (`git commit -m 'feat: add amazing feature'`)
8. Push to the branch (`git push origin feature/amazing-feature`)
9. Open a Pull Request

Please ensure your PR:
- Passes all CI checks
- Includes tests for new functionality
- Updates documentation as needed
- Follows the existing code style
- Does not introduce unsafe code without justification

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [The Rust Programming Language](https://www.rust-lang.org/)
- [Cargo](https://doc.rust-lang.org/cargo/)
- [clippy](https://github.com/rust-lang/rust-clippy)
