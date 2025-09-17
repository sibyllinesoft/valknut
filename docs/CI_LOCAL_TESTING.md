# Local CI Testing Guide

This guide explains how to run GitHub Actions locally with the same strictness and environment as the CI pipeline.

## 🎯 Problem Statement

The common issue: **"It works locally but fails in CI"**

This happens because:
- Local environments use different Rust toolchain components
- Local clippy runs with different warning levels
- Benchmarks and cross-compilation aren't tested locally
- Security audits aren't run locally
- Environment variables differ between local and CI

## 🛠️ Solutions Provided

We've created several tools to match GitHub Actions strictness exactly:

### 1. **Makefile Targets** (Recommended)

Quick and easy commands that mirror GitHub Actions jobs:

```bash
# Quick check (most common CI failures)
make gh-quick

# Individual GitHub Actions jobs
make gh-check      # Formatting, clippy, docs
make gh-test       # All test suites  
make gh-security   # Security audit
make gh-benchmarks # Benchmark compilation
make gh-cross      # Cross-compilation setup

# Complete simulation
make gh-actions    # Run all GitHub Actions jobs
```

### 2. **Act Integration**

Run actual GitHub Actions locally using [act](https://github.com/nektos/act):

```bash
# Install act (if not already installed)
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash

# Run specific jobs
make act-check     # Run 'check' job with act
make act-test      # Run 'test' job with act  
make act-all       # Run complete CI pipeline
```

### 3. **Standalone Script**

Comprehensive testing script with colored output:

```bash
# Run specific checks
./scripts/test-ci-locally.sh check
./scripts/test-ci-locally.sh test
./scripts/test-ci-locally.sh security
./scripts/test-ci-locally.sh benchmarks

# Run everything
./scripts/test-ci-locally.sh all
```

## 🔧 Configuration Files

### `.actrc` - Act Configuration
- Matches GitHub Actions environment exactly
- Uses same container images as GitHub Actions
- Sets proper environment variables
- Enables verbose logging for debugging

### `.env.act` - Environment Variables
- `GITHUB_ACTIONS=true` - Enables CI mode
- `RUSTFLAGS="-D warnings"` - Treats warnings as errors
- `CARGO_TERM_COLOR=always` - Consistent output formatting
- Matches all GitHub Actions environment variables

### `.cargo/audit.toml` - Security Audit
- Ignores known issues that can't be fixed
- Documents reasoning for each ignored advisory
- Matches CI security requirements

## 🎯 Key Differences from Default Local Development

### Clippy Strictness
```bash
# Local (lenient)
cargo clippy

# GitHub Actions (strict) 
cargo clippy --all-targets --all-features -- -D clippy::correctness -D clippy::suspicious -D clippy::complexity -W clippy::perf -W clippy::style
```

### Environment Variables
```bash
# GitHub Actions sets these automatically
export GITHUB_ACTIONS=true
export CI=true  
export RUSTFLAGS="-D warnings"
export CARGO_TERM_COLOR=always
```

### Compilation Targets
```bash
# GitHub Actions tests these compilation scenarios:
cargo check --benches                    # Benchmark compilation
cargo check --benches --features benchmarks
cargo build --all-features              # All features
cargo doc --all-features --no-deps      # Documentation
```

## 🚀 Quick Start

**Before pushing changes:**

```bash
# Quick validation (30 seconds)
make gh-quick

# Complete validation (2-3 minutes)
make gh-actions
```

**If you have act installed:**

```bash
# Test exactly what GitHub Actions will run
make act-check
```

**For debugging specific failures:**

```bash
# Just formatting and clippy
make gh-check

# Just tests
make gh-test

# Just security issues
make gh-security
```

## 🔍 Troubleshooting Common Issues

### "Clippy errors that don't appear locally"
**Cause**: Local clippy uses different warning levels  
**Solution**: Run `make gh-check` to use exact same flags as CI

### "Benchmark compilation failures"
**Cause**: Benchmarks aren't built in normal development  
**Solution**: Run `make gh-benchmarks` to test benchmark compilation

### "Security audit failures"
**Cause**: `cargo audit` not run locally  
**Solution**: Run `make gh-security` to check for vulnerabilities

### "Cross-compilation errors"  
**Cause**: Cross-compilation targets not tested locally  
**Solution**: Run `make gh-cross` to simulate cross-compilation

### "Test failures only in CI"
**Cause**: Different test environment or features  
**Solution**: Run `make gh-test` with exact CI test configuration

## 📋 Integration with Development Workflow

### Pre-commit Hook
Add to `.git/hooks/pre-commit`:
```bash
#!/bin/bash
make gh-quick
```

### VS Code Tasks
Add to `.vscode/tasks.json`:
```json
{
    "label": "GitHub Actions Check",
    "type": "shell", 
    "command": "make gh-quick",
    "group": "test"
}
```

### Git Aliases
Add to `.gitconfig`:
```ini
[alias]
    ci-check = !make gh-quick
    ci-full = !make gh-actions
```

## 🎖️ Best Practices

1. **Always run `make gh-quick` before pushing**
2. **Use `make gh-actions` for major changes**
3. **Test cross-compilation for release branches**
4. **Run security audit regularly with `make gh-security`**
5. **Use act for exact GitHub Actions simulation**

## ⚡ Performance Tips

- `gh-quick` runs the most common CI failure checks in ~30 seconds
- `gh-check` focuses on formatting/clippy issues (~1 minute)
- `gh-actions` is comprehensive but takes 2-3 minutes
- Use act only when you need exact GitHub Actions environment

## 🔗 Related Documentation

- [GitHub Actions Workflow](../.github/workflows/ci.yml)
- [Act Documentation](https://github.com/nektos/act)
- [Cargo Audit Configuration](.cargo/audit.toml)
- [Makefile Documentation](../Makefile)

---

**The goal**: Never be surprised by CI failures again! 🎯