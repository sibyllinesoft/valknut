# CLAUDE.md - Agent Efficiency Guide for Valknut

**Purpose**: Enable agents to work efficiently with the valknut Rust codebase - a high-performance code analysis engine that combines statistical analysis, graph algorithms, and AI-powered semantic evaluation.

## 🎯 Project Quick Context

**What is Valknut**: High-performance Rust implementation (~29k lines) of code analysis algorithms with multi-language support, focusing on refactorability scoring, complexity analysis, and technical debt assessment.

**Performance Focus**: SIMD-accelerated math, lock-free concurrency, async-first design, zero-cost abstractions.

---

## 📁 Project Structure Navigation

### Core Architecture (4 Main Domains)

```
src/
├── api/           # Public API & Engine Interface
│   ├── engine.rs     # ValknutEngine - main entry point 
│   ├── config_types.rs  # AnalysisConfig - high-level config
│   └── results.rs    # AnalysisResults - output structures
│
├── core/          # Analysis Algorithms & Data Structures  
│   ├── config.rs     # ValknutConfig - internal config
│   ├── pipeline/     # Multi-stage analysis pipeline
│   │   ├── pipeline_executor.rs  # AnalysisPipeline - orchestrates stages
│   │   ├── pipeline_config.rs    # Pipeline-specific config
│   │   ├── pipeline_results.rs   # Internal result structures  
│   │   └── pipeline_stages.rs    # AnalysisStages enum
│   ├── scoring.rs    # FeatureNormalizer - statistical normalization
│   ├── bayesian.rs   # BayesianNormalizer - advanced normalization
│   ├── featureset.rs # FeatureVector - core data structure
│   ├── errors.rs     # ValknutError - comprehensive error handling
│   └── file_utils.rs # File discovery and filtering
│
├── detectors/     # Specialized Analysis Algorithms
│   ├── complexity/   # Cyclomatic, cognitive complexity  
│   ├── graph/       # Dependency graphs, centrality, cycles
│   ├── lsh/         # Locality Sensitive Hashing, similarity
│   ├── structure/   # Directory organization, architectural patterns
│   ├── coverage/    # Code coverage analysis
│   ├── refactoring/ # Refactoring opportunity detection
│   └── names_simple/ # Semantic naming evaluation
│
├── lang/          # Language-Specific AST Adapters
│   ├── common/      # Shared AST utilities
│   └── python_simple/ # Python parser (tree-sitter disabled)
│
├── io/            # I/O, Persistence, Reporting
│   ├── cache/       # Result caching
│   ├── persistence/ # Database integration (optional)  
│   └── reports/     # HTML, JSON, Markdown output
│
└── bin/           # CLI Implementation
    ├── valknut.rs   # Main binary entry point
    └── cli/         # CLI modules (args, commands, output)
```

### Key Directories for Agents

- **Start Here**: `src/api/engine.rs` - ValknutEngine is the main orchestrator
- **Core Logic**: `src/core/pipeline/pipeline_executor.rs` - AnalysisPipeline runs analysis
- **Feature Extraction**: `src/core/featureset.rs` - FeatureVector is the central data structure
- **Error Handling**: `src/core/errors.rs` - ValknutError with comprehensive error types
- **CLI Interface**: `src/bin/cli/` - All CLI argument parsing and output formatting

---

## 🤖 Agent-Specific Patterns

### Which Agents Excel Where

**rust-backend-developer**: 
- Performance optimizations (SIMD, lock-free structures)
- Memory management (custom allocators, zero-copy)
- Async pipeline improvements
- Core algorithm implementations

**test-writer-fixer**:
- Unit tests: `src/core/` modules need comprehensive coverage
- Integration tests: `tests/cli_tests.rs` patterns 
- Benchmark tests: `benches/performance.rs`
- Property-based testing with `proptest` feature

**refactoring-specialist**:
- High complexity in `src/core/scoring.rs` (advanced statistics)
- Large files in `src/detectors/` (algorithm implementations)
- API surface simplification in `src/api/`

**devops-automator**:
- CI/CD integration for quality gates
- Docker containerization
- Cargo feature optimization
- Release automation

---

## 🔧 Development Workflows

### Testing Patterns

```bash
# Unit tests (fast feedback)
cargo test --lib

# Integration tests (CLI validation)  
cargo test --test cli_tests

# Benchmarks (performance validation)
cargo bench --features benchmarks

# Property-based testing
cargo test --features property-testing

# Full test suite
cargo test --all-features
```

### Build Configurations

```bash
# Development (fast compile)
cargo build

# Release (maximum performance)  
cargo build --release --features "simd,parallel,lto"

# Minimal build (CI validation)
cargo build --no-default-features

# Database integration
cargo build --features database

# Memory profiling build
cargo build --features jemalloc
```

### Common Performance Commands

```bash
# Profile with custom allocator
CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release --features jemalloc

# SIMD optimization validation
cargo build --features simd --release
objdump -d target/release/valknut | grep "vmovups\|vpaddd" # Check SIMD instructions

# Parallel processing validation
cargo test test_parallel_processing --features parallel -- --nocapture
```

---

## 🏗️ Code Architecture Insights

### Critical Abstractions

**FeatureVector** (`src/core/featureset.rs`):
```rust
// Central data structure - all analysis flows through this
pub struct FeatureVector {
    entity_id: String,
    features: HashMap<String, f64>, // feature_name -> normalized_value
    metadata: EntityMetadata,
}

// Key methods agents should understand:
impl FeatureVector {
    pub fn new(entity_id: impl Into<String>) -> Self
    pub fn add_feature(&mut self, name: &str, value: f64)
    pub fn get_feature(&self, name: &str) -> Option<f64>
    pub fn combine_with(&mut self, other: &FeatureVector, weights: &HashMap<String, f64>)
}
```

**ValknutEngine** (`src/api/engine.rs`):
```rust
// Main API - async-first design
pub struct ValknutEngine {
    pipeline: AnalysisPipeline,
    config: Arc<ValknutConfig>,
}

// Primary async methods:
impl ValknutEngine {
    pub async fn new(config: AnalysisConfig) -> Result<Self>
    pub async fn analyze_directory<P: AsRef<Path>>(&mut self, path: P) -> Result<AnalysisResults>
    pub async fn analyze_vectors(&mut self, vectors: Vec<FeatureVector>) -> Result<AnalysisResults>
}
```

**AnalysisPipeline** (`src/core/pipeline/pipeline_executor.rs`):
```rust
// Pipeline orchestrator - coordinates all analysis stages
pub struct AnalysisPipeline {
    config: AnalysisConfig,
    extractor_registry: ExtractorRegistry,
    normalizer: Option<FeatureNormalizer>,
}

// Key pipeline stages:
// 1. File Discovery -> 2. Feature Extraction -> 3. Normalization -> 4. Scoring -> 5. Results
```

### Design Patterns in Use

**Zero-Cost Abstractions**: Heavy use of generics, `const fn`, compile-time feature detection
**Async-First**: All I/O operations are async, pipeline stages run concurrently when possible  
**Builder Pattern**: Configuration objects (AnalysisConfig, ValknutConfig) use fluent builders
**Registry Pattern**: ExtractorRegistry manages feature extractors dynamically
**Type Safety**: Strong typing with custom error types, no `unwrap()` in library code

---

## 📋 Common Tasks & Pitfalls

### Frequent Agent Tasks

1. **Add New Feature Extractor**:
   - Create in `src/detectors/[category]/`
   - Register in ExtractorRegistry
   - Add integration test in pipeline
   - Update configuration schema

2. **Performance Optimization**:
   - Profile with `cargo bench`  
   - Check SIMD usage with `cargo asm`
   - Validate parallel processing with Rayon
   - Memory profiling with jemalloc/mimalloc

3. **Add Language Support**:
   - Extend `src/lang/` with parser
   - Update `src/core/config.rs` language config
   - Add feature extractors for language-specific patterns
   - Integration tests with sample code

4. **CLI Enhancement**:
   - Modify `src/bin/cli/args.rs` for argument parsing
   - Update `src/bin/cli/commands.rs` for command logic  
   - Add output formatting in `src/bin/cli/output.rs`
   - Integration tests in `tests/cli_tests.rs`

### Critical Pitfalls to Avoid

**❌ Blocking Async Runtime**: 
- Never use blocking I/O in async functions
- Use `tokio::fs` instead of `std::fs` in pipeline stages
- Wrap CPU-intensive work with `spawn_blocking`

**❌ Memory Leaks in Feature Vectors**:
- FeatureVectors can accumulate large amounts of metadata
- Always clear temporary vectors after pipeline stages
- Use streaming processing for large codebases

**❌ SIMD Compilation Issues**:
- SIMD features are compile-time only - no runtime detection
- Test on different architectures (x86_64, ARM)
- Graceful fallback when SIMD unavailable

**❌ Configuration Validation**:
- Always call `config.validate()` before using
- Configuration errors should be caught early, not during analysis
- Provide clear error messages for invalid configurations

---

## ⚡ Performance Considerations  

### SIMD Optimization Patterns

```rust
// Enable SIMD in Cargo.toml features
#[cfg(feature = "simd")]
use wide::{f64x4, f64x8};

// Vectorized normalization example from scoring.rs
pub fn normalize_batch_simd(values: &mut [f64], mean: f64, std_dev: f64) {
    #[cfg(feature = "simd")]
    {
        let mean_vec = f64x4::splat(mean);
        let std_vec = f64x4::splat(std_dev);
        
        for chunk in values.chunks_exact_mut(4) {
            let vals = f64x4::new([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let normalized = (vals - mean_vec) / std_vec;
            normalized.write_to_slice(chunk);
        }
    }
}
```

### Memory Management Patterns

```rust
// Custom allocators (feature-gated)
#[cfg(feature = "mimalloc")]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

// Lock-free concurrent access
use dashmap::DashMap;
use arc_swap::ArcSwap;

// Memory-efficient probabilistic data structures
use probabilistic_collections::HyperLogLog;
```

### Concurrency Patterns

```rust
// Parallel processing with Rayon
use rayon::prelude::*;

// Lock-free coordination
use parking_lot::RwLock;
use crossbeam::channel;

// Async coordination
use tokio::sync::{Semaphore, RwLock as TokioRwLock};
```

---

## 🔗 Integration Points

### Extending Valknut Effectively

**Feature Extractors**: Implement `FeatureExtractor` trait
```rust
pub trait FeatureExtractor: Send + Sync {
    fn extract_features(&self, source: &SourceFile) -> Result<Vec<FeatureVector>>;
    fn supported_languages(&self) -> &[&str];
    fn feature_names(&self) -> &[&str];
}
```

**Custom Normalizers**: Extend FeatureNormalizer
```rust  
impl FeatureNormalizer {
    pub fn with_custom_scheme(&mut self, scheme: Box<dyn NormalizationScheme>) -> &mut Self;
}
```

**Output Formats**: Implement in `src/io/reports/`
```rust
pub trait ReportGenerator {
    fn generate(&self, results: &AnalysisResults) -> Result<String>;
    fn content_type(&self) -> &str;
    fn file_extension(&self) -> &str;
}
```

### MCP Server Integration (In Development)

The project includes MCP (Model Control Protocol) server integration for Claude Code:

- MCP server will expose analysis capabilities
- Real-time code analysis for IDE integration  
- Refactoring recommendations with AI assistance
- Integration with `claude-code` CLI workflows

### External Tool Integration

**CI/CD Quality Gates**:
```bash
# Exit code 1 if quality thresholds not met
valknut analyze --quality-gate --max-complexity 75 --min-health 60 ./src
```

**SonarQube Integration**:
```bash  
# Generate SonarQube-compatible reports
valknut analyze --format sonarqube --out reports/ ./src
```

**Database Integration** (Optional):
```rust
#[cfg(feature = "database")]
use crate::io::persistence::DatabaseBackend;

// Store analysis results for trend analysis
let db = DatabaseBackend::new(connection_string).await?;
db.store_analysis_results(&results).await?;
```

---

## 🎯 Agent Success Patterns

### Efficient Development Flow

1. **Start with Tests**: Understand behavior through existing tests
2. **Profile First**: Use `cargo bench` before optimizing  
3. **Feature-Gate**: Use Cargo features for optional functionality
4. **Documentation**: Update rustdoc for all public APIs
5. **Integration**: Test CLI changes with `tests/cli_tests.rs`

### Configuration Management

```rust
// Always validate configuration  
let config = AnalysisConfig::default()
    .with_confidence_threshold(0.85)
    .with_max_files(1000);

// Convert to internal representation
let valknut_config = config.to_valknut_config();
valknut_config.validate()?; // Critical - catches errors early
```

### Error Handling Patterns

```rust
use crate::core::errors::{ValknutError, Result};

// Never use unwrap() in library code
// Always propagate errors with context
Err(ValknutError::validation(format!(
    "Invalid configuration: {}", details
)))

// Use error chaining for complex operations  
.map_err(|e| ValknutError::analysis("Feature extraction failed", e))?
```

---

**Key Takeaway for Agents**: Valknut prioritizes performance and correctness. Always profile changes, validate configurations early, and leverage Rust's type system for safety. The async pipeline design means most operations should be non-blocking, and the SIMD/parallel features require careful testing across architectures.
