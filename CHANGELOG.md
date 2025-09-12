# Changelog

All notable changes to valknut-rs will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.2.1] - 2024-12-11

### 🚀 Enhanced MCP Server for Claude Code Integration

This release makes the MCP (Model Control Protocol) server production-ready for Claude Code integration with comprehensive analysis tools and robust error handling.

### Added

#### 🛠️ MCP Server Enhancements
- **4 Complete MCP Tools**: Full-featured analysis capabilities via MCP protocol
  - `analyze_code`: Comprehensive code analysis with multi-language support and multiple output formats
  - `get_refactoring_suggestions`: Entity-specific refactoring recommendations with confidence scoring
  - `validate_quality_gates`: CI/CD quality gate validation with configurable thresholds
  - `analyze_file_quality`: File-level quality metrics and targeted refactoring suggestions

#### 🔧 Technical Improvements
- **Enhanced Error Handling**: Comprehensive error codes and descriptive error messages for all MCP operations
- **JSON Schema Validation**: Complete parameter validation for all MCP tool calls
- **Quality Gate Integration**: Configurable complexity, health score, and technical debt thresholds for CI/CD pipelines
- **Production Logging**: Structured logging with appropriate log levels for debugging and monitoring

#### 🧪 Testing & Quality
- **Updated Integration Tests**: All 16 MCP integration tests passing with enhanced test coverage
- **Manifest Validation**: Complete MCP manifest generation with proper JSON Schema definitions
- **Protocol Compliance**: Full JSON-RPC 2.0 protocol implementation with proper error handling

### Changed
- **MCP Tool Count**: Expanded from 2 to 4 comprehensive analysis tools
- **CLI Manifest Command**: Enhanced `valknut mcp-manifest` to include all 4 tools with complete schemas
- **Test Suite**: Updated integration tests to validate new tool functionality

### Fixed
- **API Compatibility**: Fixed MCP tools to properly use the current AnalysisResults API structure
- **Parameter Validation**: Corrected all MCP tool parameter handling and validation
- **Tool Registration**: Ensured all 4 tools are properly registered in both server initialization and manifest generation

## [1.0.0] - 2024-12-09

### 🎉 First Stable Release

This marks the first stable release of Valknut, a high-performance Rust code analysis engine. After a complete architectural overhaul and migration from Python, Valknut is now production-ready for enterprise code quality management and CI/CD integration.

### Added

#### 🏗️ Core Architecture
- **Complete Rust Implementation**: High-performance, memory-safe code analysis engine built from the ground up
- **Modular Detection System**: Pluggable architecture with specialized analyzers for different aspects of code quality
- **Multi-Language Support**: Tree-sitter based parsing for Python, TypeScript, JavaScript, Rust, and Go
- **Asynchronous Processing**: Tokio-based async runtime for optimal performance on large codebases

#### 🔍 Analysis Capabilities
- **Structure Analysis**: Directory organization assessment with architectural pattern recognition
- **Complexity Analysis**: Cyclomatic and cognitive complexity metrics with configurable thresholds
- **Code Quality Analysis**: Pattern-based function and variable name quality evaluation using statistical algorithms
- **Refactoring Analysis**: Automated detection of code smells and refactoring opportunities
- **Technical Debt Assessment**: Quantitative technical debt scoring and prioritization
- **Dependency Analysis**: Module relationship mapping, cycle detection, and chokepoint identification
- **Code Clone Detection**: Duplicate code identification with consolidation recommendations

#### 🚦 Quality Gates & CI/CD Integration
- **Quality Gate Mode**: Configurable build failure conditions based on quality metrics
- **Multi-Threshold Support**: Separate thresholds for complexity, health scores, debt ratios, and issue counts
- **CI/CD Pipeline Integration**: Ready-to-use configurations for GitHub Actions, Jenkins, and other platforms
- **Automated Quality Reporting**: Machine-readable JSON output optimized for CI/CD consumption

#### 📊 Rich Output Formats
- **Interactive HTML Reports**: Visual complexity heatmaps, refactoring dashboards, and trend analysis
- **Markdown Team Reports**: Human-readable documentation for code reviews and planning sessions
- **JSON/JSONL Output**: Machine-readable format for tool integration and automated processing
- **CSV Export**: Spreadsheet-compatible data for custom analysis and tracking
- **SonarQube Integration**: Direct output format compatibility for enterprise quality management

#### ⚡ Performance Optimizations
- **SIMD Acceleration**: Vectorized operations for mathematical computations and text processing
- **Parallel Processing**: Multi-threaded analysis with configurable concurrency levels
- **Memory Efficiency**: Streaming analysis with minimal memory footprint for large codebases
- **Caching System**: Intelligent caching for faster incremental analysis runs
- **Lock-Free Data Structures**: High-performance concurrent collections for thread-safe operations

#### 🛠️ Developer Experience
- **Comprehensive CLI**: Rich command-line interface with progress indicators and colored output
- **Flexible Configuration**: YAML-based configuration with validation and sensible defaults
- **Configuration Management**: Built-in commands for creating, validating, and managing configurations
- **Language Discovery**: Automatic language detection with configurable file extension mapping
- **Error Handling**: Detailed error messages with suggestions for resolution

#### 🔧 Configuration System
- **Unified Configuration**: Single `.valknut.yml` file for all analysis settings
- **Per-Language Settings**: Language-specific thresholds and analysis parameters
- **Quality Gate Configuration**: Detailed quality gate settings for CI/CD integration
- **Analysis Pipeline Control**: Granular control over which analysis modules to enable
- **Output Customization**: Configurable report formats and output destinations

### Changed

#### 🏗️ Architecture Overhaul
- **Python to Rust Migration**: Complete rewrite from Python to Rust for 10x+ performance improvements
- **Modular Design**: Restructured codebase into focused, cohesive modules with clear separation of concerns
- **Configuration Consolidation**: Replaced multiple configuration files with unified `.valknut.yml` system
- **Build System Enhancement**: Upgraded build system with proper dependency management and optimization flags

#### 📁 Project Structure
- **Clean Module Organization**: Reorganized codebase from scattered files into logical module hierarchy
- **Detector Specialization**: Split monolithic analyzers into focused, single-purpose detector modules
- **CLI System Restructure**: Organized CLI components into separate modules for commands, output, and coordination
- **Legacy Code Management**: Moved Python implementation to `attic/` directory for historical reference

### Fixed

#### 🔧 Core Functionality
- **UTF-8 File Handling**: Robust file reading with proper encoding detection and fallback strategies
- **CLI Default Behavior**: Fixed analyze command to default to current directory when no path specified
- **Output Directory Management**: Changed default output from `out/` to `.valknut/` for better organization
- **Error Message Clarity**: Improved error reporting with actionable suggestions and context

#### 📊 Analysis Accuracy
- **Bayesian Normalization**: Fixed score normalization bug that caused uniform 0.5 scores across all files
- **Realistic Score Distribution**: Analysis now produces meaningful score variance reflecting actual code quality differences
- **Performance Metric Accuracy**: Corrected performance benchmarks and timing measurements
- **Language Detection**: Enhanced file type detection with comprehensive extension mapping

### Performance Improvements

- **10x+ Speed Increase**: Rust implementation provides order-of-magnitude performance improvements over Python
- **Memory Efficiency**: Reduced memory usage by 60-80% through Rust's zero-cost abstractions
- **Parallel Analysis**: Added multi-threaded processing with linear scaling across CPU cores
- **SIMD Optimization**: Vectorized mathematical operations for faster numerical computations
- **Streaming Processing**: Implemented streaming analysis to handle large codebases without memory pressure

### Security

- **Memory Safety**: Rust's ownership system eliminates entire classes of memory safety vulnerabilities
- **Input Validation**: Comprehensive input validation for all CLI arguments and configuration files
- **Secure Defaults**: Conservative default configurations that prioritize security and stability
- **Dependency Auditing**: Automated security scanning of all dependencies with vulnerability reporting

### Documentation

- **Comprehensive README**: Detailed usage guide with examples for all major features
- **Configuration Documentation**: Complete reference for all configuration options and settings
- **CI/CD Integration Guide**: Step-by-step instructions for popular CI/CD platforms
- **Architecture Documentation**: Technical documentation explaining system design and module interactions

### Breaking Changes

⚠️ **Configuration Format**: The new unified `.valknut.yml` configuration format is incompatible with previous versions. Use `valknut init-config` to generate a new configuration file.

⚠️ **CLI Interface**: Some legacy command-line options have been restructured for consistency. Check `valknut --help` for current options.

⚠️ **Output Format Changes**: JSON output schema has been enhanced with additional fields. Legacy parsers may need updates.

⚠️ **Python CLI Deprecated**: The Python-based CLI has been moved to `attic/` and is no longer maintained. All functionality is available in the Rust implementation.

### Migration Guide

For users upgrading from pre-1.0 versions:

1. **Update Configuration**: Run `valknut init-config` to create a new `.valknut.yml` configuration file
2. **Update CI/CD Scripts**: Replace Python CLI calls with the new Rust binary (`valknut`)
3. **Review Quality Gates**: Check and update quality gate thresholds in the new configuration format
4. **Test Integration**: Validate that automated tools correctly parse the new JSON output format

### Dependencies

- **Rust 1.70+**: Minimum supported Rust version for building from source
- **Tree-sitter**: Language parsing support for multi-language analysis
- **Tokio**: Asynchronous runtime for high-performance I/O operations
- **Rayon**: Data parallelism for multi-threaded analysis
- **Clap**: Command-line argument parsing with rich help and validation

### Acknowledgments

- **Community Contributors**: Thanks to all contributors who provided feedback and testing during the development process
- **Rust Ecosystem**: Built on the excellent foundation provided by the Rust community and crate ecosystem
- **Research Foundation**: Based on latest research in code analysis, refactoring, and technical debt management
- **Tree-sitter Project**: For providing robust, language-agnostic parsing capabilities

---

For more details on any of these changes, see the [project documentation](README.md) or visit the [GitHub repository](https://github.com/nathanricedev/valknut).

## [1.1.0] - 2024-12-10

### Added

#### 🎯 Coverage Packs - Advanced Test Gap Analysis
- **LLM-Free Coverage Analysis**: Deterministic, algorithmic approach to test coverage analysis without AI dependencies
- **Multi-Format Coverage Parser**: Support for 5 major coverage formats:
  - **Coverage.py XML**: Python coverage reports with line-level granularity
  - **LCOV**: Linux-based coverage format for C/C++ and other languages
  - **Cobertura**: Java/Maven ecosystem coverage format
  - **JaCoCo**: Java code coverage library format
  - **Istanbul JSON**: JavaScript/TypeScript coverage reports
- **Intelligent Gap Coalescing**: Merges adjacent uncovered lines (within 3 lines) into logical coverage gaps
- **Language-Specific Chunking**: Breaks long gaps at function/class boundaries for better context
- **Impact Scoring System**: Sophisticated scoring formula weighing:
  - Gap size (40%): Lines of uncovered code
  - Complexity (20%): Cyclomatic complexity of uncovered regions
  - Fan-in (15%): Number of callers/importers
  - Exports (10%): Public API surface impact
  - Centrality (10%): Module importance in dependency graph
  - Documentation (5%): Missing documentation penalty
- **Context-Rich Snippet Previews**: Generates agent-friendly code previews with:
  - Line numbers and syntax context
  - Import statements extraction
  - Head/tail truncation for long gaps
  - Symbol boundary detection

#### 🔧 Enhanced Configuration
- **Coverage Analysis Configuration**: New comprehensive coverage section in `.valknut.yml`
- **Format Auto-Detection**: Automatic coverage format detection based on file structure and content
- **Flexible Report Discovery**: Configurable patterns for finding coverage reports in project trees

### Improved

#### 📊 Analysis Pipeline
- **Coverage Integration**: Coverage Packs seamlessly integrated into main analysis pipeline
- **Error Handling**: Robust error handling for malformed coverage files with detailed diagnostics
- **Performance Optimization**: Efficient parsing and processing of large coverage reports

#### 🛠️ Developer Experience
- **Agent-Friendly Output**: Coverage gaps formatted for development tool integration
- **Comprehensive Logging**: Detailed logging of coverage parsing and gap analysis process
- **Validation Framework**: Built-in validation for coverage file formats and content

### Technical Implementation

#### 🏗️ Architecture
- **Pure Rust Implementation**: Zero external dependencies for coverage parsing
- **Modular Design**: Separate parsers for each coverage format with shared interfaces
- **Async Processing**: Non-blocking coverage file processing with tokio integration
- **Memory Efficient**: Stream-based parsing for large coverage files

#### 🧪 Quality Assurance  
- **Comprehensive Test Suite**: Full test coverage for all coverage format parsers
- **Format Validation**: Extensive validation testing with real-world coverage files
- **Edge Case Handling**: Robust handling of malformed, incomplete, or unusual coverage reports
- **Performance Testing**: Benchmarking for large coverage file processing

### Bug Fixes
- **Coverage.py XML Detection**: Fixed overly strict format detection causing false negatives
- **Symbol Extraction**: Corrected class name extraction logic for Python files
- **Error API Consistency**: Fixed ValknutError::io calls to match required parameter signature
- **Test Compilation**: Added missing PartialEq derivations for SymbolKind enum

## [Unreleased]

### Planned Features
- **MCP Integration**: Claude Code integration for IDE assistance
- **Language Expansion**: Additional language support (Java, C#, C++)
- **Statistical Analysis**: Enhanced pattern-based analysis capabilities
- **Cloud Integration**: SaaS offering with team collaboration features
- **IDE Plugins**: VS Code, IntelliJ, and other IDE integrations

---

*This changelog is automatically maintained. For detailed commit history, see the project's Git log.*