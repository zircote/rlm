## Summary

<!-- Brief description of the changes in this PR -->

## Related Issues

<!-- Link any related issues: Fixes #123, Relates to #456 -->

## Changes

<!-- List the key changes made in this PR -->

-
-
-

## Type of Change

<!-- Mark the relevant option with an [x] -->

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Documentation update
- [ ] Refactoring (no functional changes)
- [ ] Performance improvement
- [ ] Test coverage improvement

## Testing

<!-- Describe how you tested these changes -->

### Test Commands Run

```bash
cargo test --all-features
cargo clippy --all-targets --all-features
cargo fmt --check
cargo doc --no-deps
cargo deny check
```

### Manual Testing

<!-- Describe any manual testing performed -->

## Checklist

<!-- Mark completed items with an [x] -->

### Code Quality

- [ ] My code follows the project's code style (rustfmt)
- [ ] I have run `cargo clippy` and fixed all warnings
- [ ] I have run `cargo fmt` to format my code
- [ ] No unsafe code is added (or it is properly documented and justified)
- [ ] No `unwrap()`, `expect()`, or `panic!()` in library code

### Testing

- [ ] I have added tests that prove my fix/feature works
- [ ] New and existing tests pass locally with `cargo test`
- [ ] I have added doc tests for new public APIs

### Documentation

- [ ] I have updated the documentation accordingly
- [ ] I have added doc comments (`///`) for new public items
- [ ] Doc comments include examples where appropriate
- [ ] I have updated the CHANGELOG.md (if applicable)

### Supply Chain

- [ ] I have run `cargo deny check` and resolved any issues
- [ ] New dependencies are justified and from trusted sources
- [ ] No new security advisories are introduced

### Commit Hygiene

- [ ] My commits follow conventional commit format
- [ ] I have rebased on the latest main branch
- [ ] I have squashed fixup commits

## API Changes

<!-- If this PR changes the public API, describe the changes here -->

### Before

```rust
// Previous API (if applicable)
```

### After

```rust
// New API (if applicable)
```

## Performance Impact

<!-- If applicable, describe any performance implications -->

## Screenshots

<!-- If applicable, add screenshots to help explain your changes -->

## Additional Notes

<!-- Add any additional context about the PR here -->
