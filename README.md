<div align="center">
  <img src="assets/logo.webp" alt="Valknut Logo" width="200">

  **High-Performance Code Analysis for Modern Development Teams**
</div>

Valknut provides comprehensive code analysis through advanced statistical algorithms and graph-based analysis. While other tools count lines and check syntax, Valknut analyzes code complexity, identifies architectural debt, and provides actionable refactoring recommendations with quantified impact. Built in Rust for production speed and integrated with CI/CD for automated quality gates.

**Stop guessing what needs refactoring. Get data-driven insights that improve code maintainability.**

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Quickstart

### Installation

#### Via Homebrew (macOS)

```bash
brew tap sibyllinesoft/valknut
brew install valknut
```

#### Via Cargo (recommended)

```bash
cargo install valknut-rs
```

#### Build from Source (requires Rust 1.70+)

```bash
git clone https://github.com/sibyllinesoft/valknut
cd valknut
cargo build --release
```

### Get Results in 30 Seconds

```bash
# Analyze your codebase and get actionable insights
./target/release/valknut analyze ./src

# Generate team-friendly HTML report
valknut analyze --format html --out reports/ ./src

# Set up CI/CD quality gates (fails build if thresholds exceeded)
valknut analyze --quality-gate --max-complexity 75 --min-health 60 ./src
```

**That's it.** Valknut will analyze your code structure, complexity, naming quality, and technical debt, then provide prioritized recommendations for improvement.

## What Makes Valknut Different

### Statistical Code Analysis
Traditional tools analyze syntax. Valknut analyzes **patterns and complexity**. It uses advanced statistical algorithms to evaluate code structure, identify complexity hotspots, and detect architectural anti-patterns that impact maintainability.

### Production-Ready Performance
Built in Rust with SIMD optimizations, Valknut analyzes large codebases in seconds, not minutes. Designed for enterprise-scale projects with 100k+ files while maintaining sub-linear memory usage.

### Quantified Technical Debt
Get concrete metrics on technical debt with prioritized recommendations. Know exactly which refactoring will provide the highest impact and where to focus your team's effort.

### Zero-Configuration CI/CD Integration
Drop into any CI/CD pipeline with quality gates that fail builds when code quality degrades. No complex setup required.

### Multi-Language Support
Comprehensive structural analysis for Python, TypeScript, JavaScript, Rust, Go, and more. Each language parser understands syntactic patterns and provides language-specific complexity insights.

## Core Capabilities

**Structure Analysis**: Identifies architectural anti-patterns and organizational debt that impacts maintainability

**Complexity Intelligence**: Goes beyond cyclomatic complexity to measure cognitive load and refactoring priority

**Code Quality Analysis**: Statistical evaluation of code patterns and structural complexity

**Refactoring Recommendations**: Actionable insights with quantified impact scoring and effort estimation  

**Dependency Health**: Detects circular dependencies, architectural chokepoints, and coupling hotspots

**Technical Debt Quantification**: Measurable debt metrics with ROI analysis for refactoring efforts

## Configuration

### Quick Setup

```bash
# Generate default configuration
valknut init-config --output .valknut.yml

# Validate configuration
valknut validate-config --config .valknut.yml

# View all available options
valknut print-default-config
```

### Quality Gates for CI/CD

Configure automatic build failures when quality thresholds are exceeded:

```yaml
quality_gates:
  enabled: true
  max_complexity: 75        # Fail if complexity score exceeds 75
  min_health: 60           # Fail if health score drops below 60
  max_debt: 30             # Fail if technical debt exceeds 30%
  max_issues: 50           # Fail if more than 50 total issues
  max_critical: 0          # Fail on any critical issues
```

### Language-Specific Configuration

```yaml
languages:
  python:
    enabled: true
    complexity_threshold: 10.0
    file_extensions: [".py", ".pyi"]
  typescript:
    enabled: true
    complexity_threshold: 10.0
    file_extensions: [".ts", ".tsx"]
```

### Advanced Options

```yaml
analysis:
  enabled: true
  complexity_threshold: 10.0
  min_confidence: 0.65     # Analysis confidence threshold
  include_test_files: true # Include test files in analysis
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Code Quality Gate
on: [push, pull_request]

jobs:
  quality-gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Valknut
        run: |
          # Install from crates.io
          cargo install valknut-rs
      
      - name: Run Quality Gate
        run: |
          valknut analyze \
            --quality-gate \
            --max-complexity 75 \
            --min-health 60 \
            --format html \
            --out quality-reports/ \
            ./src
      
      - name: Upload Reports
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: quality-reports
          path: quality-reports/
```

### Jenkins Pipeline

```groovy
pipeline {
    agent any
    stages {
        stage('Code Quality Gate') {
            steps {
                sh '''
                    valknut analyze \
                      --quality-gate \
                      --max-issues 50 \
                      --max-critical 0 \
                      --format sonar \
                      ./src
                '''
            }
        }
    }
}
```

### Development Workflow Integration

```bash
# Pre-commit hook
valknut analyze --fail-on-issues ./src

# Code review preparation  
valknut analyze --format markdown ./src > REVIEW.md

# Continuous monitoring
valknut analyze --format json ./src | jq '.health_score'
```

## Advanced Usage

### Output Formats

```bash
# Interactive HTML reports for teams
valknut analyze --format html --out reports/ ./src

# Machine-readable JSON for automation
valknut analyze --format json ./src

# Markdown reports for documentation  
valknut analyze --format markdown ./src

# CSV data for spreadsheet analysis
valknut analyze --format csv ./src

# SonarQube integration format
valknut analyze --format sonar ./src
```

### Advanced Analysis Options

```bash
# Custom configuration
valknut analyze --config custom.yml ./src

# Specific analysis types
valknut analyze --skip-naming --skip-refactoring ./src

# Large codebase optimization
valknut analyze --max-files 50000 --parallel 8 ./src

# Language-specific analysis
valknut list-languages
```

### Legacy Command Support

```bash
# Structure-only analysis
valknut structure ./src --format pretty

# Dependency and clone analysis
valknut impact ./src --cycles --clones
```

## Contributing & Development

### Quick Development Setup

```bash
git clone https://github.com/sibyllinesoft/valknut
cd valknut

# Build and test
cargo build
cargo test

# Install language parsers
./scripts/install_parsers.sh

# Run on sample project
cargo run -- analyze ./test_data/sample_python --format json
```

### Project Architecture

Valknut uses a modular pipeline architecture:
- **Core Pipeline**: Orchestrates multi-stage analysis with caching
- **Language Parsers**: Tree-sitter based AST analysis for each supported language
- **Statistical Analysis**: Advanced algorithms for code complexity evaluation
- **Report Generation**: Templated output in multiple formats
- **Quality Gates**: Configurable thresholds for CI/CD integration

### Contributing

We welcome contributions! Please:
1. Add tests for new features
2. Run `cargo clippy` and `cargo fmt` before submitting
3. Update documentation for user-facing changes
4. Benchmark performance-critical changes
5. Follow Rust best practices and idioms

See [docs/](docs/) for detailed architecture documentation and design decisions.

## Supported Languages

Currently supported languages with full structural analysis:
- **Python** - Comprehensive AST analysis with async/await pattern detection
- **TypeScript/JavaScript** - Modern ES features, React patterns, Node.js idioms
- **Rust** - Ownership analysis, zero-cost abstraction patterns
- **Go** - Concurrency patterns, interface analysis
- **Java** - OOP patterns, enterprise frameworks
- **C/C++** - Memory management, performance patterns

Additional languages supported for basic complexity analysis. See `valknut list-languages` for the complete list.

## Performance

Benchmarked on real-world codebases:
- **100k+ files**: < 30 seconds full analysis
- **Memory usage**: < 2GB for large monorepos
- **Parallel processing**: Scales linearly with CPU cores
- **Incremental analysis**: 5x faster on subsequent runs

## License

MIT License - see [LICENSE](LICENSE) file for details.

---

**Ready to improve your code quality?** Start with `valknut analyze ./src` and get actionable insights in seconds.