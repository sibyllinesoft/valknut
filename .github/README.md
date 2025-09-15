# GitHub Actions CI/CD System

This directory contains a comprehensive CI/CD system for the Valknut project, designed to enforce quality standards, prevent regressions, and automate releases.

## 🚀 Workflow Overview

### Core Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| **[CI](.github/workflows/ci.yml)** | Push/PR to main/develop | Core testing, building, and quality gates |
| **[Quality Gates](.github/workflows/quality-gates.yml)** | Push/PR to main/develop | Advanced code quality enforcement |
| **[Security](.github/workflows/security.yml)** | Push/PR + nightly | Security scanning and vulnerability detection |
| **[Performance](.github/workflows/performance.yml)** | Push/PR + nightly | Performance testing and regression detection |
| **[Release](.github/workflows/release.yml)** | Version tags | Automated release creation and publishing |

### Quality Enforcement Strategy

Our CI/CD system implements a multi-layered quality enforcement strategy:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Fast Checks   │───▶│  Deep Analysis  │───▶│ Comprehensive   │
│                 │    │                 │    │   Validation    │
│ • Format        │    │ • Security      │    │ • Integration   │
│ • Clippy        │    │ • Performance   │    │ • Cross-platform│
│ • Basic tests   │    │ • Memory leaks  │    │ • Feature matrix│
└─────────────────┘    └─────────────────┘    └─────────────────┘
        ~2 min              ~10 min              ~20 min
```

## 📋 Quality Standards Enforced

### Code Quality
- **Zero Warnings**: Clippy warnings cause build failure
- **Format Consistency**: rustfmt formatting enforced
- **Error Handling**: No raw `map_err` patterns, proper `ValknutError` usage
- **No Unsafe Code**: Without proper `// SAFETY:` documentation
- **Test Coverage**: Minimum 80% line coverage required

### Security Standards
- **Dependency Audit**: cargo-audit for known vulnerabilities
- **Supply Chain**: License compliance and dependency risk analysis
- **Code Scanning**: GitHub CodeQL for vulnerability detection
- **Secret Detection**: Prevents hardcoded secrets in code

### Performance Standards
- **Benchmark Regression**: 120% performance degradation threshold
- **Memory Leak Detection**: Valgrind validation on Linux
- **SIMD Validation**: Ensures SIMD optimizations are effective
- **Stress Testing**: Large project analysis validation

### Architecture Standards
- **Module Organization**: Enforces proper module structure
- **Documentation**: Comprehensive rustdoc for all public APIs
- **No Duplicates**: Detects and prevents code duplication
- **Error Patterns**: Enforces consistent error handling patterns

## 🔧 Configuration Files

### [dependabot.yml](dependabot.yml)
Automated dependency management with intelligent grouping:
- **Weekly Updates**: Scheduled for Monday mornings
- **Grouped Updates**: Similar dependencies grouped to reduce PR noise
- **Security Priority**: Automatic security updates
- **Version Pinning**: Major versions require manual review

### [CODEOWNERS](CODEOWNERS)
Comprehensive code ownership ensuring all changes are reviewed:
- **API Changes**: Core API requires careful review
- **Security Files**: CI/CD and security configs need approval
- **Performance Critical**: Algorithm implementations need review
- **Documentation**: Ensures docs stay up to date

### Issue Templates
Structured issue reporting for better bug tracking:
- **[Bug Reports](ISSUE_TEMPLATE/bug_report.yml)**: Comprehensive bug information collection
- **[Feature Requests](ISSUE_TEMPLATE/feature_request.yml)**: Detailed feature proposal format
- **[Performance Issues](ISSUE_TEMPLATE/performance_issue.yml)**: Performance problem reporting

### [Pull Request Template](pull_request_template.md)
Comprehensive PR checklist ensuring quality submissions:
- **Quality Checks**: Formatting, clippy, documentation
- **Testing**: Coverage impact and test validation
- **Security**: Input validation and audit compliance
- **Performance**: Benchmark impact assessment

## 🎯 Workflow Details

### CI Workflow
The main CI workflow runs comprehensive validation:

```yaml
Jobs:
  check:          # Fast feedback (2-3 min)
    - Format check (rustfmt)
    - Clippy warnings (zero tolerance)
    - Documentation build
    
  test:           # Cross-platform testing (5-10 min)
    - Linux/macOS/Windows on stable/beta/MSRV
    - Coverage reporting (80% minimum)
    - Feature matrix testing
    
  audit:          # Security validation (2-5 min)
    - Dependency vulnerabilities
    - License compliance
    - Outdated dependency detection
    
  integration:    # Real-world validation (3-5 min)
    - CLI integration tests
    - Multi-format output validation
    - Quality gate enforcement
```

### Quality Gates Workflow
Advanced code quality enforcement:

```yaml
Jobs:
  error-handling-patterns:
    - No raw map_err usage
    - Proper ValknutError patterns
    - No unwrap in library code
    
  code-organization:
    - attic/ directory isolation
    - Module structure validation
    - File size limits
    
  duplicate-detection:
    - Unused dependency detection
    - Duplicate pattern identification
    - Constant deduplication
    
  documentation-coverage:
    - Public API documentation
    - README completeness
    - CHANGELOG validation
```

### Security Workflow
Comprehensive security scanning:

```yaml
Jobs:
  security-audit:
    - cargo-audit for vulnerabilities
    - cargo-deny for policy compliance
    - Yanked crate detection
    
  codeql:
    - GitHub CodeQL analysis
    - Security vulnerability detection
    
  supply-chain:
    - Unsafe code detection (cargo-geiger)
    - License compliance checking
    - Dependency risk analysis
    
  dependency-scan:
    - Trivy vulnerability scanner
    - SARIF report generation
```

### Performance Workflow
Performance validation and regression detection:

```yaml
Jobs:
  benchmark:
    - Criterion.rs benchmarks
    - Performance regression detection
    - Memory usage profiling
    
  simd-performance:
    - SIMD vs scalar comparison
    - Optimization effectiveness validation
    
  parallel-performance:
    - Single vs multi-threaded comparison
    - Scalability validation
    
  memory-leak-detection:
    - Valgrind analysis
    - Memory safety validation
    
  stress-testing:
    - Large project analysis (2000+ files)
    - Resource usage monitoring
    - Performance threshold validation
```

### Release Workflow
Automated release creation and publishing:

```yaml
Jobs:
  validate-release:
    - Version consistency checking
    - CHANGELOG validation
    
  build-release:
    - Multi-platform binary builds
    - Checksum generation
    - Artifact preparation
    
  create-release:
    - GitHub release creation
    - Release notes generation
    - Binary distribution
    
  publish-crates:
    - crates.io publishing (optional)
    - Version validation
```

## 🚦 Quality Gates

### Mandatory Gates
These gates must pass for any PR to be merged:

1. **Format & Lint**: Code must be properly formatted with no clippy warnings
2. **Test Coverage**: Minimum 80% line coverage required
3. **Security Audit**: No known vulnerabilities in dependencies
4. **Error Handling**: Proper ValknutError usage patterns
5. **Documentation**: Complete rustdoc for public APIs

### Performance Gates
Performance regressions trigger failures:

1. **Benchmark Regression**: >20% performance degradation
2. **Memory Usage**: Memory leak detection must pass
3. **Stress Testing**: Large project analysis must complete

### Security Gates
Security issues cause immediate failures:

1. **Vulnerability Scan**: No critical or high-severity vulnerabilities
2. **Unsafe Code**: Proper documentation for any unsafe blocks
3. **Dependency Audit**: All dependencies must be secure and licensed appropriately

## 🔄 Automated Processes

### Dependency Management
- **Weekly Updates**: Dependabot creates grouped PRs for dependency updates
- **Security Updates**: Automatic security patches
- **Version Pinning**: Major updates require manual review

### Performance Monitoring
- **Nightly Benchmarks**: Performance tracked over time
- **Regression Alerts**: Automated alerts for performance degradation
- **Memory Profiling**: Regular memory usage validation

### Release Automation
- **Tag-based Releases**: Version tags trigger automatic releases
- **Multi-platform Builds**: Binaries for Linux, macOS, Windows, ARM64
- **Changelog Integration**: Automatic release notes from CHANGELOG.md

## 🛡️ Security Features

### Vulnerability Detection
- **Daily Scans**: Nightly security scans for new vulnerabilities
- **SARIF Integration**: Security findings integrated with GitHub Security tab
- **Supply Chain**: Analysis of dependency security and licensing

### Code Security
- **Secret Scanning**: Detection of hardcoded secrets
- **Unsafe Code Audit**: Validation of unsafe Rust code usage
- **Input Validation**: Enforcement of proper input validation patterns

## 📊 Monitoring and Reporting

### Coverage Reporting
- **Codecov Integration**: Coverage tracking and reporting
- **Threshold Enforcement**: 80% minimum coverage requirement
- **Historical Tracking**: Coverage trends over time

### Performance Tracking
- **Benchmark History**: Performance trends tracked in GitHub Pages
- **Memory Usage**: Memory profiling results stored as artifacts
- **Regression Alerts**: Automatic notifications for performance issues

### Security Reporting
- **Vulnerability Dashboard**: GitHub Security tab integration
- **Audit Reports**: Regular security audit summaries
- **Compliance Tracking**: License and dependency compliance monitoring

## 🎮 Local Development

### Running CI Checks Locally

```bash
# Format check
cargo fmt --all -- --check

# Clippy check
cargo clippy --all-targets --all-features -- -D warnings

# Test with coverage
cargo tarpaulin --all-features --out xml

# Security audit
cargo audit

# Full feature matrix test
cargo test --all-features
cargo test --no-default-features
```

### Performance Testing
```bash
# Run benchmarks
cargo bench --features benchmarks

# Memory profiling
valgrind --leak-check=full ./target/release/valknut analyze ./test-project

# SIMD validation
RUSTFLAGS="-C target-cpu=native" cargo build --release --features simd
```

## 🔮 Future Enhancements

### Planned Improvements
- **Fuzzing Integration**: Continuous fuzzing for security and reliability
- **Property-Based Testing**: Enhanced test coverage with proptest
- **Docker Integration**: Containerized testing environments
- **MCP Server Testing**: Integration testing for Claude Code MCP server

### Metrics to Add
- **Code Complexity**: Cyclomatic complexity tracking
- **Technical Debt**: Automated technical debt assessment
- **Documentation Coverage**: Percentage of documented APIs
- **Performance Baselines**: Per-feature performance baselines

---

This CI/CD system ensures that Valknut maintains high quality standards while enabling rapid development and reliable releases. All workflows are designed to provide fast feedback while comprehensively validating code quality, security, and performance.