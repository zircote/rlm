# RLM-RS Makefile
# Common build, test, and development tasks

.PHONY: all build release test test-verbose test-lib lint lint-strict format format-check deny doc-check check check-strict dev quick install clean doc bench ci msrv verify-clean help

# Default target
all: check

# Build debug binary
build:
	cargo build --all-features --locked

# Build release binary
release:
	cargo build --release --all-features --locked

# Run all tests (excluding usearch-hnsw due to segfault in FFI cleanup)
test:
	cargo test --features fastembed-embeddings --locked

# Run tests with output
test-verbose:
	cargo test --features fastembed-embeddings --locked -- --nocapture

# Run library tests only
test-lib:
	cargo test --lib --features fastembed-embeddings --locked

# Run clippy linting (warnings allowed)
lint:
	cargo clippy --all-targets --all-features --locked

# Run clippy linting (warnings as errors - for CI)
lint-strict:
	cargo clippy --all-targets --all-features --locked -- -D warnings

# Run supply chain security audit
deny:
	cargo deny check

# Build documentation (without opening)
doc-check:
	cargo doc --no-deps --all-features --locked

# Format code
format:
	cargo fmt

# Check formatting without modifying
format-check:
	cargo fmt -- --check

# Full check (format + lint + test)
check: format-check lint test

# Full check with strict linting (format + lint-strict + test + doc + deny)
check-strict: format-check lint-strict test doc-check deny

# Development workflow: check then install
dev: check install

# Quick build and install (skip tests)
quick: build install

# Install to ~/.cargo/bin
install:
	cargo install --path . --all-features --force

# Clean build artifacts
clean:
	cargo clean

# Generate and open documentation
doc:
	cargo doc --no-deps --open

# Run benchmarks (quick validation mode for CI)
bench:
	@echo "Benchmarks not yet configured for rlm-rs"

# Run full benchmarks (for performance analysis)
bench-full:
	@echo "Benchmarks not yet configured for rlm-rs"

# MSRV check - verify builds with minimum supported Rust version
msrv:
	@MSRV=$$(grep '^rust-version' Cargo.toml | cut -d'"' -f2); \
	echo "Checking MSRV: $$MSRV"; \
	rustup run $$MSRV cargo check --all-features --all-targets --locked

# Ensure working tree is clean before CI/release.
# Default: warn locally, enforce in CI or when REQUIRE_CLEAN=1.
verify-clean:
	@if [ "$$GITHUB_ACTIONS" = "true" ] || [ "$(REQUIRE_CLEAN)" = "1" ]; then \
		git diff --quiet && git diff --cached --quiet || (echo "Working tree is dirty"; exit 1); \
	else \
		if ! (git diff --quiet && git diff --cached --quiet); then \
			echo "Working tree is dirty (continuing)"; \
		fi; \
	fi

# CI-style full check (all gates must pass)
# Matches GitHub Actions: fmt, clippy, test, doc, deny, msrv
ci: verify-clean format-check lint-strict test doc-check deny msrv build release

# Publish to crates.io (dry run first)
publish-dry:
	cargo publish --dry-run

# Publish to crates.io
publish: check-strict
	cargo publish

# Show help
help:
	@echo "RLM-RS Makefile Targets:"
	@echo ""
	@echo "  Build:"
	@echo "    build          Build debug binary"
	@echo "    release        Build release binary"
	@echo "    install        Install to ~/.cargo/bin"
	@echo "    clean          Clean build artifacts"
	@echo ""
	@echo "  Test:"
	@echo "    test           Run all tests"
	@echo "    test-verbose   Run tests with output"
	@echo "    test-lib       Run library tests only"
	@echo ""
	@echo "  Quality:"
	@echo "    lint           Run clippy linting (warnings allowed)"
	@echo "    lint-strict    Run clippy with warnings as errors"
	@echo "    format         Format code"
	@echo "    format-check   Check formatting"
	@echo "    deny           Run supply chain security audit"
	@echo "    doc-check      Build documentation"
	@echo "    msrv           Check MSRV (minimum supported Rust version)"
	@echo "    check          Full check (format + lint + test)"
	@echo "    check-strict   Strict check (format + lint-strict + test + doc + deny)"
	@echo "    ci             CI-style full check (all gates must pass)"
	@echo ""
	@echo "  Workflows:"
	@echo "    dev            Full check then install"
	@echo "    quick          Build and install (skip tests)"
	@echo "    doc            Generate and open documentation"
	@echo ""
	@echo "  Publishing:"
	@echo "    publish-dry    Dry run publish to crates.io"
	@echo "    publish        Publish to crates.io"
	@echo ""
	@echo "  Installation (after publishing):"
	@echo "    cargo install rlm-rs"
	@echo "    brew install zircote/tap/rlm-rs"
	@echo ""
	@echo "  CI Gates (run 'make ci' before pushing):"
	@echo "    1. cargo fmt -- --check"
	@echo "    2. cargo clippy --all-targets --all-features -- -D warnings"
	@echo "    3. cargo test --all-features"
	@echo "    4. cargo doc --no-deps"
	@echo "    5. cargo deny check"
	@echo "    6. rustup run \$$MSRV cargo check --all-features"
