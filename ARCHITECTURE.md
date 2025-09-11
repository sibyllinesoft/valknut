# Valknut Architecture Documentation

This document describes the architecture, design patterns, and implementation details of the Valknut code analysis system.

## Table of Contents
- [Overview](#overview)
- [System Architecture](#system-architecture)
- [Core Components](#core-components)
- [Analysis Pipeline](#analysis-pipeline)
- [Language Support](#language-support)
- [Data Flow](#data-flow)
- [Performance Considerations](#performance-considerations)
- [Extension Points](#extension-points)
- [Design Decisions](#design-decisions)

## Overview

Valknut is a high-performance code analysis tool implemented in Rust that provides comprehensive analysis capabilities including:

- **Structure Analysis**: Directory organization and file distribution assessment
- **Complexity Analysis**: AST-based complexity metrics using Tree-sitter parsers
- **Code Quality Analysis**: Statistical pattern evaluation and naming convention assessment
- **Technical Debt Assessment**: Quantitative debt scoring and prioritization
- **Refactoring Recommendations**: Actionable improvement suggestions with impact analysis
- **Quality Gates**: CI/CD integration with configurable failure conditions

## System Architecture

### High-Level Architecture

```
┌─────────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│      CLI Layer      │    │     API Layer       │    │   Configuration     │
│   (bin/valknut.rs)  │◄──►│   (api/*.rs)        │◄──►│   (valknut.yml)    │
└─────────────────────┘    └─────────────────────┘    └─────────────────────┘
           │                         │                          │
           ▼                         ▼                          ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Core Analysis Pipeline                               │
│                        (core/pipeline.rs)                                  │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Detector Modules                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │  Structure  │  │ Complexity  │  │ Refactoring │  │   Names     │        │
│  │  Analysis   │  │  Analysis   │  │  Analysis   │  │  Analysis   │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Language Adapters                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │   Python    │  │ TypeScript  │  │    Rust     │  │     Go      │        │
│  │   Parser    │  │   Parser    │  │   Parser    │  │   Parser    │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           I/O Layer                                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │   Reports   │  │    Cache    │  │ Persistence │  │   Config    │        │
│  │ Generation  │  │  System     │  │   Layer     │  │  Management │        │
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Analysis Pipeline (`core/pipeline.rs`)

The central orchestrator that coordinates all analysis activities:

```rust
pub struct AnalysisPipeline {
    config: AnalysisConfig,
    complexity_analyzer: ComplexityAnalyzer,
    structure_extractor: StructureExtractor,
    refactoring_analyzer: RefactoringAnalyzer,
}

impl AnalysisPipeline {
    pub async fn analyze_paths(
        &self, 
        paths: &[PathBuf],
        progress_callback: Option<ProgressCallback>,
    ) -> Result<ComprehensiveAnalysisResult>
}
```

**Key Responsibilities:**
- File discovery and filtering
- Coordinating detector execution
- Progress tracking and reporting  
- Result aggregation and health metrics calculation
- Quality gate evaluation

### 2. Detector Modules (`detectors/`)

#### Structure Analyzer (`detectors/structure.rs`)
- Directory organization analysis
- File size and distribution assessment
- Reorganization recommendations

#### Complexity Analyzer (`detectors/complexity.rs`)
- AST-based complexity metrics
- Cyclomatic and cognitive complexity
- Maintainability index calculation

#### Code Quality Analyzer (`detectors/names_simple/`)
- Statistical pattern-based quality assessment
- Function and variable naming evaluation
- Renaming suggestions with context

#### Refactoring Analyzer (`detectors/refactoring.rs`)
- Code smell detection
- Improvement opportunity identification
- Impact assessment and prioritization

### 3. Language Adapters (`lang/`)

Language-specific parsers and AST analyzers using Tree-sitter:

```rust
pub trait LanguageAdapter {
    fn parse_file(&self, content: &str) -> Result<Tree>;
    fn extract_entities(&self, tree: &Tree) -> Vec<CodeEntity>;
    fn calculate_complexity(&self, node: &Node) -> ComplexityMetrics;
    fn analyze_names(&self, entities: &[CodeEntity]) -> Vec<NamingIssue>;
}
```

**Supported Languages:**
- **Python** (`lang/python.rs`) - Full support
- **TypeScript** (`lang/typescript.rs`) - Full support  
- **JavaScript** (`lang/javascript.rs`) - Full support
- **Rust** (`lang/rust_lang.rs`) - Full support
- **Go** (`lang/go.rs`) - Experimental

### 4. I/O Layer (`io/`)

#### Report Generation (`io/reports.rs`)
- Multiple output format support (JSON, HTML, Markdown, CSV)
- Template-based report generation
- Interactive dashboard creation

#### Caching System (`io/cache.rs`)
- File-based analysis result caching
- Incremental analysis support
- Cache invalidation strategies

#### Configuration Management (`core/config.rs`)
- YAML configuration parsing and validation
- Default value management
- CLI option integration

## Analysis Pipeline

### 1. File Discovery Phase

```rust
async fn discover_files(&self, paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    // 1. Traverse input paths
    // 2. Filter by file extensions
    // 3. Exclude specified directories  
    // 4. Apply file limits if configured
}
```

**Filtering Rules:**
- Include files matching configured extensions
- Exclude directories like `node_modules`, `target`, `.git`
- Respect max file limits for large codebases
- Handle both files and directories as input

### 2. Structure Analysis Phase

```rust
async fn run_structure_analysis(&self, paths: &[PathBuf]) -> Result<StructureAnalysisResults> {
    // 1. Analyze directory organization
    // 2. Identify overcrowded directories
    // 3. Detect large files needing splitting
    // 4. Generate reorganization recommendations
}
```

**Analysis Types:**
- **Directory Pressure**: Files/LOC per directory thresholds
- **File Size Analysis**: Large file identification and splitting suggestions
- **Balance Analysis**: Code distribution assessment
- **Partitioning Recommendations**: Structural improvement suggestions

### 3. Complexity Analysis Phase

```rust
async fn run_complexity_analysis(&self, files: &[PathBuf]) -> Result<ComplexityAnalysisResults> {
    // 1. Parse files using Tree-sitter
    // 2. Extract code entities (functions, classes)
    // 3. Calculate complexity metrics
    // 4. Identify complexity hotspots
}
```

**Metrics Calculated:**
- **Cyclomatic Complexity**: Decision point counting
- **Cognitive Complexity**: Human perception-based complexity
- **Technical Debt Score**: Quantified maintainability assessment
- **Maintainability Index**: Composite maintainability score

### 4. Quality Analysis Phase

```rust
async fn run_quality_analysis(&self, files: &[PathBuf]) -> Result<QualityAnalysisResults> {
    // 1. Extract function and variable names
    // 2. Analyze naming quality using statistical patterns
    // 3. Detect naming inconsistencies
    // 4. Generate improvement suggestions
}
```

**Statistical Analysis:**
- **Pattern Recognition**: Advanced algorithms for code quality assessment
- **Inconsistency Detection**: Statistical name quality assessment
- **Suggestion Generation**: Pattern-based improvement recommendations
- **API Protection**: Preserve public API naming conventions

### 5. Refactoring Analysis Phase

```rust
async fn run_refactoring_analysis(&self, files: &[PathBuf]) -> Result<RefactoringAnalysisResults> {
    // 1. Detect code smells and anti-patterns
    // 2. Identify improvement opportunities
    // 3. Calculate impact and effort estimates
    // 4. Prioritize recommendations
}
```

**Opportunities Identified:**
- Extract Method opportunities
- Extract Class candidates
- Reduce complexity suggestions
- Remove duplication recommendations

### 6. Health Metrics Calculation

```rust
fn calculate_health_metrics(&self, ...) -> HealthMetrics {
    // 1. Aggregate analysis results
    // 2. Calculate composite scores
    // 3. Normalize metrics to 0-100 scale
    // 4. Weighted health score calculation
}
```

**Health Score Components:**
- **Maintainability Score** (30% weight): Based on maintainability index
- **Structure Quality Score** (30% weight): Based on structural issues
- **Complexity Score** (20% weight): Inverse of complexity metrics  
- **Technical Debt Score** (20% weight): Inverse of debt ratio

## Language Support

### Tree-sitter Integration

Valknut uses Tree-sitter for robust, language-agnostic parsing:

```rust
use tree_sitter::{Language, Parser, Tree};

pub struct LanguageParser {
    language: Language,
    parser: Parser,
}

impl LanguageParser {
    pub fn parse(&mut self, source: &str) -> Option<Tree> {
        self.parser.parse(source, None)
    }
}
```

**Benefits:**
- **Error Recovery**: Robust parsing of incomplete/malformed code
- **Language Agnostic**: Uniform AST structure across languages
- **Performance**: Fast parsing with minimal memory usage
- **Incremental**: Support for incremental parsing (future)

### Language-Specific Implementations

#### Python Support (`lang/python.rs`)
- Function, class, and method extraction
- Import and module analysis
- Python-specific complexity patterns
- Django/Flask framework awareness

#### TypeScript/JavaScript Support (`lang/typescript.rs`, `lang/javascript.rs`)
- Modern syntax support (ES2024, TypeScript 5.x)
- React component analysis
- Module system understanding
- Node.js and browser pattern recognition

#### Rust Support (`lang/rust_lang.rs`)
- Trait and implementation analysis
- Ownership and borrowing pattern recognition
- Cargo project structure awareness
- Async/await pattern analysis

## Data Flow

### Input Processing
```
CLI Args → Configuration → Path Discovery → File Filtering
```

### Analysis Flow
```
Files → Language Detection → AST Parsing → Entity Extraction → 
Metric Calculation → Issue Detection → Recommendation Generation
```

### Output Generation
```
Analysis Results → Format Selection → Template Processing → 
Report Generation → File Output
```

### Caching Flow
```
Input Hash → Cache Check → [Cache Hit: Return | Cache Miss: Analyze → Cache Store]
```

## Performance Considerations

### Rust Performance Features

1. **Zero-Cost Abstractions**: Traits and generics compile to efficient code
2. **Memory Safety**: No garbage collection overhead
3. **SIMD Optimizations**: Vectorized operations for large datasets
4. **Parallel Processing**: Rayon for data parallelism

### Optimization Strategies

#### Parallel Analysis
```rust
use rayon::prelude::*;

files.par_iter()
    .map(|file| analyze_file(file))
    .collect::<Result<Vec<_>, _>>()
```

#### Memory Efficiency
```rust
// Stream processing for large files
use std::io::{BufReader, BufRead};

let reader = BufReader::new(file);
for line in reader.lines() {
    // Process line by line
}
```

#### Caching Strategy
- **Content-based hashing**: SHA-256 of file content + config
- **Hierarchical caching**: File, directory, and project level
- **TTL-based expiration**: Configurable cache lifetime
- **Selective invalidation**: Only invalidate affected cache entries

## Extension Points

### Adding New Languages

1. **Language Definition**:
```rust
pub struct NewLanguageAdapter {
    parser: LanguageParser,
    config: LanguageConfig,
}

impl LanguageAdapter for NewLanguageAdapter {
    // Implement required methods
}
```

2. **Register Language**:
```rust
pub fn register_languages() -> HashMap<String, Box<dyn LanguageAdapter>> {
    let mut languages = HashMap::new();
    languages.insert("newlang".to_string(), Box::new(NewLanguageAdapter::new()));
    languages
}
```

### Adding New Detectors

1. **Detector Implementation**:
```rust
pub struct CustomDetector {
    config: CustomConfig,
}

impl Detector for CustomDetector {
    async fn analyze(&self, entities: &[CodeEntity]) -> Result<DetectorResult> {
        // Custom analysis logic
    }
}
```

2. **Pipeline Integration**:
```rust
impl AnalysisPipeline {
    async fn run_custom_analysis(&self, files: &[PathBuf]) -> Result<CustomAnalysisResults> {
        let detector = CustomDetector::new(self.config.custom.clone());
        detector.analyze(&entities).await
    }
}
```

### Adding New Output Formats

1. **Format Implementation**:
```rust
pub struct CustomFormatter;

impl ReportFormatter for CustomFormatter {
    fn format(&self, results: &ComprehensiveAnalysisResult) -> Result<String> {
        // Custom formatting logic
    }
}
```

2. **Registration**:
```rust
pub fn get_formatter(format: &OutputFormat) -> Box<dyn ReportFormatter> {
    match format {
        OutputFormat::Custom => Box::new(CustomFormatter),
        // ... other formats
    }
}
```

## Design Decisions

### ADR-001: Rust Implementation Choice

**Status**: Accepted

**Context**: Need for high-performance code analysis tool capable of handling large codebases.

**Decision**: Implement in Rust for performance, memory safety, and ecosystem benefits.

**Consequences**:
- ✅ Excellent performance and memory efficiency
- ✅ Memory safety without garbage collection overhead
- ✅ Rich ecosystem of parsing and analysis crates
- ❌ Higher learning curve for contributors
- ❌ Longer compilation times during development

### ADR-002: Tree-sitter for Parsing

**Status**: Accepted

**Context**: Need robust, language-agnostic parsing for multiple programming languages.

**Decision**: Use Tree-sitter for all language parsing.

**Consequences**:
- ✅ Uniform AST structure across languages
- ✅ Error-tolerant parsing of incomplete code
- ✅ High performance with incremental parsing
- ✅ Large ecosystem of language grammars
- ❌ Additional dependency on Tree-sitter binaries
- ❌ Learning curve for Tree-sitter query syntax

### ADR-003: Multi-Stage Analysis Pipeline

**Status**: Accepted

**Context**: Need to coordinate multiple types of analysis with progress tracking.

**Decision**: Implement pipeline pattern with distinct analysis stages.

**Consequences**:
- ✅ Clear separation of concerns
- ✅ Easy to add new analysis types
- ✅ Progress tracking and error isolation
- ✅ Configurable analysis stages
- ❌ Potential for redundant file processing
- ❌ More complex coordination logic

### ADR-004: Configuration-First Design

**Status**: Accepted

**Context**: Need flexible configuration for different project types and team standards.

**Decision**: Make all analysis behavior configurable through YAML configuration.

**Consequences**:
- ✅ Flexible adaptation to different codebases
- ✅ Team-specific standard enforcement
- ✅ CLI and config file integration
- ✅ Validation and documentation
- ❌ Configuration complexity for users
- ❌ Default configuration maintenance

### ADR-005: Quality Gates for CI/CD

**Status**: Accepted

**Context**: Need automated quality control integration with development workflows.

**Decision**: Implement configurable quality gates with exit codes for CI/CD.

**Consequences**:
- ✅ Automated quality enforcement
- ✅ Configurable failure conditions
- ✅ Integration with existing CI/CD systems
- ✅ Gradual quality improvement enforcement
- ❌ Potential for overly strict gates blocking development
- ❌ Configuration complexity for optimal thresholds

## Future Considerations

### Planned Enhancements

1. **Incremental Analysis**: Only analyze changed files
2. **Language Server Protocol**: IDE integration for real-time analysis
3. **Machine Learning**: Improved pattern recognition and recommendations
4. **Distributed Analysis**: Support for analyzing very large codebases
5. **Custom Rules**: User-defined analysis rules and patterns
6. **Integration APIs**: REST API for third-party tool integration

### Scalability Plans

1. **Database Backend**: Store analysis results in database for large projects
2. **Caching Strategy**: Distributed caching for team environments
3. **Parallel Execution**: Multi-machine analysis coordination
4. **Memory Management**: Streaming analysis for memory-constrained environments

This architecture provides a solid foundation for comprehensive code analysis while maintaining flexibility for future enhancements and extensions.