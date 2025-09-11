# Contributing to Valknut

We welcome contributions to Valknut! This document provides guidelines for contributing to this high-performance Rust code analysis engine.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Code Style](#code-style)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)
- [Community Guidelines](#community-guidelines)

## Getting Started

### Prerequisites

- **Rust**: 1.75+ (latest stable recommended)
- **Git**: For version control
- **System Requirements**: Linux, macOS, or Windows with WSL2

### Quick Setup

```bash
# Clone the repository
git clone https://github.com/nathanricedev/valknut.git
cd valknut

# Build the project
cargo build

# Run tests
cargo test

# Install for local development
cargo install --path .
```

## Development Setup

### Development Dependencies

```bash
# Install additional development tools
cargo install cargo-audit          # Security auditing
cargo install cargo-machete        # Unused dependency detection
cargo install cargo-outdated       # Dependency update checking
```

### Build Configurations

```bash
# Debug build (default)
cargo build

# Release build (optimized)
cargo build --release

# Build with all features
cargo build --all-features

# Build with specific features
cargo build --features "database,benchmarks"
```

## Making Changes

### Branch Naming Conventions

- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation updates
- `refactor/description` - Code refactoring
- `perf/description` - Performance improvements

### Commit Message Format

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification:

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

Examples:
```
feat(detector): add new complexity analysis algorithm
fix(core): resolve memory leak in scoring engine  
docs(api): update configuration examples
perf(lsh): optimize hash computation with SIMD
```

### Code Organization

- **Core Logic**: `src/core/` - Fundamental algorithms and data structures
- **Detectors**: `src/detectors/` - Specialized analysis modules
- **Language Support**: `src/lang/` - Language-specific parsers and adapters
- **I/O Operations**: `src/io/` - File handling, caching, and reporting
- **Public API**: `src/api/` - High-level user-facing interfaces
- **CLI**: `src/bin/` - Command-line interface implementation

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test core::scoring

# Run integration tests only
cargo test --test integration

# Run with coverage (requires additional tools)
cargo tarpaulin --out html
```

### Test Guidelines

1. **Unit Tests**: Test individual functions and methods
2. **Integration Tests**: Test component interactions
3. **Property Tests**: Use `proptest` for algorithmic validation
4. **Performance Tests**: Benchmark critical paths

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_function_name() {
        // Arrange
        let input = create_test_input();
        
        // Act
        let result = function_under_test(input);
        
        // Assert
        assert_eq!(result.expected_field, expected_value);
    }
    
    #[test]
    #[should_panic(expected = "specific error message")]
    fn test_error_conditions() {
        function_that_should_panic(invalid_input);
    }
}
```

## Code Style

### Rust Style Guidelines

We follow the official Rust style guidelines with additional project-specific conventions:

```bash
# Format code
cargo fmt

# Check style and common issues
cargo clippy

# Check with pedantic lints
cargo clippy -- -W clippy::pedantic
```

### Documentation Requirements

- **Public APIs**: Must have comprehensive documentation
- **Complex Algorithms**: Include implementation notes and complexity analysis
- **Configuration**: Document all options with examples
- **Examples**: Provide working code examples

```rust
/// Calculates complexity scores using advanced statistical normalization.
/// 
/// This function implements a hybrid approach combining Bayesian inference
/// with robust statistical measures to handle outliers effectively.
/// 
/// # Arguments
/// 
/// * `metrics` - Raw complexity metrics from code analysis
/// * `config` - Configuration parameters for normalization
/// 
/// # Returns
/// 
/// Returns a `Result<ComplexityScore>` with normalized scores or an error
/// if the input data is insufficient for reliable analysis.
/// 
/// # Examples
/// 
/// ```rust
/// use valknut_rs::{ComplexityMetrics, ScoringConfig};
/// 
/// let metrics = ComplexityMetrics::new(cyclomatic, cognitive, nesting);
/// let config = ScoringConfig::default();
/// let score = calculate_complexity_score(&metrics, &config)?;
/// ```
/// 
/// # Performance
/// 
/// Time complexity: O(n log n) where n is the number of metrics
/// Space complexity: O(n) for intermediate calculations
pub fn calculate_complexity_score(
    metrics: &ComplexityMetrics,
    config: &ScoringConfig,
) -> Result<ComplexityScore> {
    // Implementation...
}
```

### Performance Considerations

- **Use SIMD**: Leverage SIMD operations for mathematical computations
- **Memory Efficiency**: Prefer stack allocation and avoid unnecessary clones
- **Async I/O**: Use async operations for file processing
- **Parallel Processing**: Use `rayon` for CPU-intensive tasks

```rust
// Good: SIMD-optimized computation
use wide::f64x4;
let values = f64x4::from([a, b, c, d]);
let result = values.fast_max();

// Good: Efficient iteration
results.par_iter()
    .filter_map(|item| item.valid_metric())
    .collect()

// Avoid: Unnecessary allocations
let owned_string = input.to_string(); // Only if ownership needed
let borrowed = &input; // Prefer borrowing when possible
```

## Pull Request Process

### Before Submitting

1. **Run Tests**: Ensure all tests pass locally
2. **Check Linting**: Fix all clippy warnings
3. **Update Documentation**: Update relevant documentation
4. **Add Tests**: Include tests for new functionality
5. **Update Changelog**: Add entry to `CHANGELOG.md`

### PR Requirements

- **Clear Description**: Explain what changes were made and why
- **Linked Issues**: Reference related issues using `Fixes #123` or `Closes #123`
- **Small Scope**: Keep changes focused on a single feature/fix
- **Clean History**: Squash commits into logical units

### PR Template

```markdown
## Description
Brief description of the changes and their purpose.

## Type of Change
- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Documentation update
- [ ] Performance improvement
- [ ] Code refactoring

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing performed
- [ ] Performance impact assessed

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] Tests added/updated and passing
- [ ] Changelog updated
```

### Review Process

1. **Automated Checks**: CI/CD pipeline runs automatically
2. **Code Review**: Maintainers review code quality and design
3. **Testing**: Additional testing may be requested
4. **Approval**: At least one maintainer approval required
5. **Merge**: Squash and merge into main branch

## Release Process

### Version Numbers

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR.MINOR.PATCH** (e.g., 1.2.3)
- **Major**: Breaking changes
- **Minor**: New features, backwards compatible
- **Patch**: Bug fixes, backwards compatible

### Release Steps

1. **Update Version**: Increment version in `Cargo.toml`
2. **Update Changelog**: Finalize changelog entries
3. **Create Tag**: `git tag v1.2.3`
4. **Push Tag**: Automated release process triggers
5. **Publish Crate**: Released to crates.io automatically

## Community Guidelines

### Code of Conduct

We are committed to providing a welcoming and inclusive environment. Please:

- **Be Respectful**: Treat all contributors with respect
- **Be Constructive**: Provide helpful feedback and suggestions
- **Be Patient**: Remember that everyone has different skill levels
- **Be Professional**: Keep discussions focused on technical matters

### Getting Help

- **Documentation**: Check the extensive documentation first
- **Issues**: Search existing issues before creating new ones
- **Discussions**: Use GitHub Discussions for questions and ideas
- **Discord**: Join our community Discord for real-time help

### Reporting Issues

When reporting bugs, please include:

1. **Environment**: OS, Rust version, valknut version
2. **Reproduction Steps**: Minimal example to reproduce the issue
3. **Expected Behavior**: What you expected to happen
4. **Actual Behavior**: What actually happened
5. **Error Messages**: Complete error messages and stack traces
6. **Configuration**: Relevant configuration files

### Security Issues

For security vulnerabilities, please:

1. **Do NOT** create a public issue
2. **Email**: Send details to security@sibyllinesoft.com
3. **Include**: Detailed description and reproduction steps
4. **Response**: We aim to respond within 48 hours

## Recognition

Contributors are recognized in several ways:

- **Changelog**: Significant contributions mentioned in release notes
- **Contributors File**: Added to CONTRIBUTORS.md
- **Special Thanks**: Major contributions highlighted in releases

Thank you for contributing to Valknut! Your efforts help make code analysis faster, more accurate, and more accessible for developers worldwide.