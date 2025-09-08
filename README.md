<div align="center">
  <img src="logo.webp" alt="Valknut Logo" width="200">

  
  **High-performance code structure analyzer implemented in Rust**
</div>

Valknut is a fast, efficient code structure analyzer that identifies refactoring opportunities in your codebase. Built in Rust for optimal performance, it analyzes directory structures, file organization, and code distribution to provide actionable recommendations for improving code maintainability.

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Quick Start

### Installation

Build from source (requires Rust):

```bash
git clone https://github.com/your-org/valknut
cd valknut
cargo build --release
```

The binary will be available at `target/release/valknut`.

### Basic Usage

```bash
# Analyze code structure in a directory
valknut structure /path/to/your/code

# Pretty output format
valknut structure /path/to/your/code --format pretty

# Use custom configuration
valknut structure /path/to/your/code --config valknut-config.yml

# Limit to top recommendations
valknut structure /path/to/your/code --top 10 --verbose
```

### Configuration

Valknut uses YAML configuration files to customize analysis behavior. See `valknut-config.yml` for the default configuration structure.

### Command Line Interface

For detailed CLI usage, see [CLI_USAGE.md](CLI_USAGE.md).

## Features

- **⚡ High Performance**: Implemented in Rust for maximum speed and efficiency
- **📁 Structure Analysis**: Analyzes directory organization and file distribution patterns
- **🎯 Actionable Recommendations**: Identifies specific refactoring opportunities
- **📊 Multiple Output Formats**: JSON, pretty-printed, and configurable output
- **🔧 Configurable**: Extensive YAML-based configuration system
- **🚀 Production Ready**: Reliable analysis suitable for CI/CD integration
- **🔍 Multi-language Awareness**: Understands common project structures across languages

## Architecture

### Core Analysis Engine

Valknut's Rust-based analysis engine focuses on:

1. **Directory Structure Analysis** - Identifies overcrowded directories and imbalanced hierarchies
2. **File Size Analysis** - Detects files that are too large or have grown beyond maintainable sizes
3. **Organization Patterns** - Recognizes common project organization issues
4. **Partitioning Recommendations** - Suggests how to split large modules or reorganize directories

### Analysis Types

| Analysis | Focus | Output |
|----------|-------|--------|
| **Branch Packs** | Directory organization | Recommendations for splitting directories |
| **File Split Packs** | Large file identification | Suggestions for breaking up large files |
| **Balance Analysis** | Code distribution | Insights into uneven code organization |
| **Clustering** | Related code grouping | Suggestions for better code organization |

### Configuration Options

Valknut provides extensive configuration through `valknut-config.yml`:

- **Structure Analysis Settings**: Configure thresholds for directory and file analysis
- **Partitioning Parameters**: Control clustering and balance algorithms
- **Output Formatting**: Customize report generation and formatting

## Why Valknut?

### 🔥 Key Benefits

**⚡ Rust Performance**: Built from the ground up in Rust for maximum speed and memory efficiency.

**🏗️ Structure-Focused**: Unlike traditional linters that focus on syntax, Valknut analyzes your codebase's organizational structure.

**📊 Actionable Insights**: Provides specific, ranked recommendations for improving code organization.

**🎯 Maintainability First**: Identifies structural issues that impact long-term code maintainability.

**🔧 Configurable Analysis**: Extensive configuration options to tailor analysis to your project's needs.

## Configuration

### Structure Analysis Configuration (`valknut-config.yml`)

```yaml
# Valknut Structure Analysis Configuration
structure:
  # Enable/disable analysis types
  enable_branch_packs: true
  enable_file_split_packs: true
  
  # Maximum number of top recommendations to return
  top_packs: 20

# File system directory analysis settings
fsdir:
  # Maximum files per directory before considering it overcrowded
  max_files_per_dir: 25
  
  # Maximum subdirectories per directory before pressure
  max_subdirs_per_dir: 10
  
  # Maximum lines of code per directory before pressure
  max_dir_loc: 2000
  
  # Minimum imbalance gain required for branch recommendation (0.0-1.0)
  min_branch_recommendation_gain: 0.15

# File system file analysis settings  
fsfile:
  # Lines of code threshold for considering files "huge"
  huge_loc: 800
  
  # Byte size threshold for considering files "huge" 
  huge_bytes: 128000
  
  # Minimum lines of code before considering file split
  min_split_loc: 200

# Graph partitioning and clustering settings
partitioning:
  # Balance tolerance for partitioning (0.25 = ±25%)
  balance_tolerance: 0.25
  
  # Maximum number of clusters per partition
  max_clusters: 4
```

## Performance

- **High-throughput analysis**: Built in Rust for optimal performance
- **Memory efficient**: Minimal memory footprint during analysis
- **Scalable**: Handles large codebases efficiently
- **Fast startup**: Quick analysis without lengthy initialization

## Python Version (Archived)

The original Python implementation of Valknut has been archived to `attic/python-valknut/`. This version included comprehensive code analysis features like:

- Multi-language AST parsing
- Complexity metrics and clone detection  
- MCP server integration
- LLM-ready refactor briefs

The Python version served as the foundation for the current Rust implementation, which focuses specifically on code structure analysis with improved performance.

### Migration from Python Version

If you were using the Python version:

1. **Structure Analysis**: The Rust version focuses on structure analysis rather than comprehensive code metrics
2. **Performance**: Significantly faster analysis with the Rust implementation
3. **Configuration**: New YAML-based configuration system (see `valknut-config.yml`)
4. **CLI**: Updated command-line interface (see [CLI_USAGE.md](CLI_USAGE.md))

## Development

### Setup

```bash
git clone https://github.com/your-org/valknut
cd valknut
cargo build
```

### Testing

```bash
# Run tests
cargo test

# Run with output
cargo test -- --nocapture

# Run benchmarks
cargo bench
```

### Adding Features

1. Create feature branch (`git checkout -b feature/amazing-feature`)
2. Make changes and add tests
3. Ensure all tests pass (`cargo test`)
4. Run benchmarks to verify performance (`cargo bench`)
5. Commit changes (`git commit -m 'Add amazing feature'`)
6. Push to branch (`git push origin feature/amazing-feature`)
7. Open Pull Request

## Contributing

Contributions are welcome! Please read our contributing guidelines and submit pull requests to the main repository.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Rust community for excellent tooling and libraries
- Original Python implementation that served as the foundation
- Research in code structure analysis and refactoring patterns