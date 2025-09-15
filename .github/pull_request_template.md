# Pull Request

## Summary
<!-- Provide a brief description of the changes -->

## Type of Change
<!-- Check all that apply -->
- [ ] 🐛 Bug fix (non-breaking change which fixes an issue)
- [ ] ✨ New feature (non-breaking change which adds functionality)
- [ ] 💥 Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] 📚 Documentation update
- [ ] 🎨 Code style/formatting changes
- [ ] ♻️ Refactoring (no functional changes)
- [ ] ⚡ Performance improvement
- [ ] 🧪 Test changes
- [ ] 🔧 Build/CI changes
- [ ] 🔒 Security fix

## Changes Made
<!-- Describe the changes in detail -->

### Core Changes
- 
- 
- 

### Files Modified
<!-- List key files changed and why -->
- `src/path/to/file.rs` - 
- `tests/test_file.rs` - 

## Testing
<!-- Describe the testing performed -->

### Test Coverage
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Benchmark tests added/updated (if performance-related)
- [ ] Manual testing performed

### Test Commands Run
```bash
# List the test commands you ran
cargo test
cargo test --all-features
cargo bench  # if applicable
```

### Coverage Impact
<!-- If you know the coverage impact -->
- Coverage before: X%
- Coverage after: Y%
- New lines covered: Z

## Quality Checks
<!-- Check all that completed successfully -->

### Code Quality
- [ ] `cargo fmt` - Code is formatted
- [ ] `cargo clippy -- -D warnings` - No clippy warnings
- [ ] `cargo doc` - Documentation builds without warnings
- [ ] No `unwrap()` or `expect()` in library code (except tests)
- [ ] Proper error handling with `ValknutError`

### Performance (if applicable)
- [ ] No performance regressions detected
- [ ] Memory usage acceptable
- [ ] SIMD optimizations tested (if relevant)
- [ ] Parallel processing validated (if relevant)

### Security
- [ ] No hardcoded secrets or sensitive data
- [ ] Input validation implemented for new code
- [ ] `cargo audit` passes
- [ ] No new unsafe code (or properly documented with `// SAFETY:`)

## Documentation
<!-- Check all that apply -->
- [ ] Code comments added/updated
- [ ] API documentation updated (rustdoc)
- [ ] README updated (if needed)
- [ ] CHANGELOG.md updated (if needed)
- [ ] Examples updated (if API changed)

## Breaking Changes
<!-- If this is a breaking change, describe the impact -->

### Impact
- [ ] API changes
- [ ] Configuration format changes
- [ ] CLI interface changes
- [ ] Behavior changes

### Migration Guide
<!-- Provide guidance for users upgrading -->
```rust
// Before
old_api_usage()

// After  
new_api_usage()
```

## Performance Impact
<!-- If performance-related changes -->

### Benchmarks
<!-- Include benchmark results if available -->
```
test_name: 
  Before: X ns/iter
  After:  Y ns/iter
  Change: ±Z%
```

### Memory Usage
<!-- If memory usage changed -->
- Memory usage impact: 
- Peak memory: 

## Related Issues
<!-- Link to issues this PR addresses -->
- Fixes #issue_number
- Closes #issue_number
- Related to #issue_number

## Checklist
<!-- Final verification before requesting review -->

### Pre-submission
- [ ] Self-review completed
- [ ] All CI checks pass locally
- [ ] Tests are focused and test the right things
- [ ] No debug print statements left in code
- [ ] Commit messages are clear and follow conventions

### Review Ready
- [ ] Ready for review
- [ ] Needs discussion (mark as draft if so)
- [ ] Waiting for dependency/blocker

## Additional Notes
<!-- Any additional information for reviewers -->

### Reviewer Focus Areas
<!-- What should reviewers pay special attention to? -->
- 
- 

### Known Limitations
<!-- Any known issues or limitations -->
- 
- 

### Future Work
<!-- Related work that should be done in future PRs -->
- 
- 

---

<!-- 
Review Guidelines for Reviewers:

1. **Quality Standards**: Ensure code follows Valknut's quality standards
2. **Performance**: Check for performance implications
3. **Security**: Verify security best practices
4. **Error Handling**: Confirm proper error handling patterns
5. **Testing**: Validate test coverage and quality
6. **Documentation**: Check documentation completeness
7. **Breaking Changes**: Understand impact on users

Key Files to Review:
- Core API changes: `src/api/`
- Algorithm changes: `src/core/`, `src/detectors/`
- Error handling: Look for proper `ValknutError` usage
- Performance: Check for SIMD/parallel optimizations
- Tests: Ensure comprehensive coverage
-->