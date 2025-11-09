# Valknut Architecture Deep Dive – November 2025

## Scope & Methodology
- Reviewed the Rust workspace (`Cargo.toml`) plus the bundled `doc_audit` crate and CLI entrypoints.
- Focused on how the pipeline composes detectors, how configuration propagates, and how language services and outputs are wired.
- Cross-referenced code in `src/core`, `src/detectors`, `src/lang`, `src/bin/cli`, and supporting docs to understand the intended contract.
- Highlighted the highest-impact gaps that affect modularity, maintainability, or user-facing accuracy.

## Current Architecture Snapshot
- **CLI layer** (`src/bin/cli`) merges config layers, exposes commands (`analyze`, `doc-audit`, `list-languages`, `mcp-*`) and feeds `ValknutEngine`.
- **API layer** (`src/api`) adapts CLI config into `ValknutConfig`, instantiates `AnalysisPipeline`, and normalizes `AnalysisResults`.
- **Core pipeline** (`src/core/pipeline`) handles file discovery, batched I/O, arena entity extraction, detector orchestration, scoring, and report shaping.
- **Detectors** (`src/detectors`) implement structure analysis, complexity, dependency graph metrics, LSH clone detection, coverage ingestion, and refactoring heuristics.
- **Language adapters** (`src/lang`) provide Tree-sitter based parsing for Python, JS/TS, Rust, and Go. Everything else should fall back to “unsupported”.
- **Auxiliary subsystems** include `AstService` (central parsing/cache), `ArenaFileAnalyzer` (allocation-friendly entity extraction), report generators, and the AI oracle (Gemini 2.5 Pro via `reqwest`).

## Key Findings & Recommendations
| # | Area | Risk | Recommendation |
|---|------|------|----------------|
| 1 | Pipeline & stage orchestration | Monolithic `AnalysisPipeline` hard-codes detector wiring, ignores user config for most analyzers, and duplicates LSH setup paths (`src/core/pipeline/pipeline_executor.rs:45-153`). | Extract a builder/registry that instantiates stages from `ValknutConfig`, honor per-detector config structs, and isolate file discovery/summary logic into dedicated services. |
| 2 | Configuration propagation | Three overlapping configs (`api::AnalysisConfig`, `core::config::AnalysisConfig`, `core::pipeline::AnalysisConfig`) drift. For example, complexity analysis cannot be disabled because pipeline config forces it to `true` (`src/core/pipeline/pipeline_config.rs:107-150`). | Collapse to a single canonical representation or generate the downstream config via `TryFrom<&ValknutConfig>` without loss of fidelity; add tests to prevent drift. |
| 3 | Data flow between arena, AST, and detectors | `run_complexity_analysis_from_arena_results` immediately re-reads every file because `ArenaAnalysisResult` drops source text (`src/core/pipeline/pipeline_stages.rs:300-340`). This negates the arena optimization and doubles I/O. | Extend `ArenaAnalysisResult` to hold an `Arc<str>` or pooled bytes slice, or allow detectors to consume the AST cache built during arena extraction. Couple with `AstService` to avoid reparsing. |
| 4 | Language support contract | Only five adapters exist (`src/lang/registry.rs:45-55`), yet `FileReader::is_code_file` and the CLI language table both claim support for Java/C++/C#/etc. (`src/core/file_utils.rs:118-151`, `src/bin/cli/commands.rs:1259-1360`). Users hitting those extensions will get “unsupported language” errors late in the run. | Derive CLI output, README content, and file discovery extension filters directly from the adapter registry; treat Go as “beta” until feature parity exists. |
| 5 | Detector module hygiene | `src/detectors/mod.rs:33-39` publicly declares an `embedding` module that does not exist in the tree, which breaks `cargo check`. Structure/coverage detectors instantiated in the pipeline always use `StructureConfig::default()` / `CoverageConfig::default()` regardless of user settings (`src/core/pipeline/pipeline_executor.rs:102-142`). | Remove stale modules or restore the missing crate; thread the configured structs into `AnalysisStages::new*` so tuning knobs in `valknut.yml` actually have effect. |
| 6 | Reporting & orchestration | `AnalysisPipeline::analyze_paths` mixes file discovery, batching, detector scheduling, progress reporting, scoring, and quality gates in one async method >400 lines long (`src/core/pipeline/pipeline_executor.rs:175-420`). This makes guarded changes risky and prevents plug-in stages (e.g., doc coverage) from reusing the infrastructure. | Split into `FileDiscoveryService`, `BatchReader`, `StageScheduler`, and `ReportAssembler` traits; wire them through `ExtractorRegistry` (already defined but unused) so new detectors can register themselves without modifying the pipeline monolith. |

## Detailed Observations
### 1. Pipeline construction ignores configurable analyzers
- `AnalysisPipeline::new_with_config` instantiates every analyzer with `StructureConfig::default()`, `ComplexityConfig::default()`, etc., even though `ValknutConfig` ships user-provided values (`src/core/pipeline/pipeline_executor.rs:82-153`).
- `AnalysisStages::new*` further overwrites coverage and complexity configs with `Default::default()` (`src/core/pipeline/pipeline_stages.rs:209-247`).
- **Recommendation:** Introduce a `PipelineComponents` builder that accepts `&ValknutConfig` and only falls back to defaults when the user did not set a value. Thread the chosen config back into detectors for parity with the Python implementation. Regression tests should assert that toggling `structure.top_packs` or coverage paths influences results.

### 2. Missing modular boundaries in the pipeline
- `AnalysisPipeline::analyze_paths` performs: git-aware discovery, batched reads, arena analysis, two `futures::join!` groups, summary calculation, health metrics, and gating inside one function. Cross-cutting change requests (e.g., streaming analysis) require editing the same method.
- The documented `ExtractorRegistry` (`pub use pipeline_executor::{AnalysisPipeline, ExtractorRegistry, ...};`) is never actually populated (`analysis_pipeline.extractor_registry().get_all_extractors()` always returns 0, per `tests`).
- **Recommendation:** Define traits such as `FileDiscovery`, `BatchReader`, `Stage`, `AggregationPass`, and allow the registry to provide enabled stages based on config. This unlocks better testability (mock a stage) and makes integrating new detectors a registration problem instead of a pipeline surgery.

### 3. Arena/AST data duplication
- `ArenaAnalysisResult` currently contains entities but not the source text or AST handles, forcing detectors to go back to disk (`src/core/pipeline/pipeline_stages.rs:310-336`).
- This defeats the “read once, analyze many times” design of `read_files_batched` and adds latency under slow I/O.
- **Recommendation:**
  1. Add an optional `Arc<CachedTree>` or pooled source slice to `ArenaAnalysisResult`.
  2. Let `AstService` accept an existing tree when scoring/complexity detectors run.
  3. Add benchmarks to prove the arena path no longer triggers a second read.

### 4. Config duplication & loss of fidelity
- CLI args → `api::AnalysisConfig` → `core::config::ValknutConfig` → `core::pipeline::AnalysisConfig` require three conversions with overlapping fields, each with its own defaults.
- Example: `core::pipeline::AnalysisConfig::from(ValknutConfig)` explicitly sets `enable_complexity_analysis = true` regardless of user intent, and flattens `exclude_patterns` into inferred directory names (`src/core/pipeline/pipeline_config.rs:107-150`).
- **Recommendation:** Collapse the pipeline config into a lightweight view (`PipelineOptions`) sourced directly from `ValknutConfig`, or auto-generate conversions using `serde` derive + `#[serde(default)]` to avoid manual duplication. Add a snapshot test that toggles every CLI flag and asserts the resulting `AnalysisPipeline.config` matches expectations.

### 5. Language/catalog inconsistency
- Only Python/JS/TS/Rust/Go adapters exist (`src/lang/registry.rs:45-55`).
- File discovery (`FileReader::is_code_file`, `pipeline_config.file_extensions`) and CLI output advertise Java, C++, C#, etc. – languages that will immediately raise `ValknutError::unsupported`. This is visible when running `valknut list-languages`, which shows eight “supported” languages even though half do not compile.
- **Recommendation:**
  - Drive file extension filters and CLI tables from the adapter registry (`LanguageRegistry::all()`), tagging Go as “beta”.
  - Update docs/README (see below) to reflect true support to avoid user churn.
  - Add a smoke test that `list-languages` matches the adapters compiled into the binary.

### 6. Detector module hygiene
- `src/detectors/mod.rs` exports an `embedding` module that is absent, which prevents `cargo check` from succeeding on a clean clone.
- The structure detector is configurable but the pipeline never injects the user’s `StructureConfig`, and coverage extraction is always created with defaults, ignoring `valknut.yml` overrides (`src/core/pipeline/pipeline_executor.rs:102-142`, `src/core/pipeline/pipeline_stages.rs:209-247`).
- **Recommendation:** Fix the module list immediately (either add the missing module or drop the export). When constructing `AnalysisStages`, pass `valknut_config.structure.clone()` / `valknut_config.coverage.clone()` to the respective extractors so YAML edits work. Add regression tests covering custom structure thresholds and coverage search paths.

### 7. Reporting & CLI layering opportunities
- The CLI already supports doc audits, MCP servers, and the Gemini-based refactoring oracle, but these are only loosely mentioned in docs. Aligning the documentation with the actual commands will reduce surprises and help developers discover the tooling ecosystem baked into the repo.
- Consider extracting a shared “command metadata” table so new commands automatically appear in README/help text.

## Quick Wins (1–2 sprints)
1. **Fix build blockers:** remove or restore `detectors::embedding`, wire pipeline stages to use `ValknutConfig` values, and ensure `cargo check` succeeds on CI with the current tree.
2. **Align language reporting with reality:** derive CLI/README lists from `lang::registry` and narrow the default extension filters so unsupported files do not enter the pipeline.
3. **Tighten config conversions:** add targeted tests proving that disabling `--no-structure` or `analysis.modules.structure=false` actually propagates to `AnalysisPipeline`. Fix the “complexity always enabled” bug as part of that work.
4. **Preserve source/AST data across stages:** extend `ArenaAnalysisResult` and `AnalysisStages` to avoid re-reading files, then measure the latency drop on a medium-sized repo.

## Medium-Term Roadmap
- **Modular pipeline refactor:** introduce a stage registry and service traits so detectors can be added/removed without editing 400-line methods. Use this to enable streaming or per-language scheduling later.
- **Unified configuration schema:** treat `ValknutConfig` as the source of truth and derive CLI + pipeline views from it using `serde` conversions. Document the schema once and generate CLI help from it to reduce drift.
- **Language plug-in model:** wrap adapters in a trait + registry that can be extended via feature flags or optional crates, making it obvious how to add new languages without touching the core.
- **Instrumentation:** expose per-stage timings (currently only logged at a coarse level) so CI users can see where time is spent and validate optimizations.

## References
- `src/core/pipeline/pipeline_executor.rs`, `src/core/pipeline/pipeline_stages.rs`
- `src/core/pipeline/pipeline_config.rs`, `src/core/config.rs`
- `src/lang/registry.rs`, `src/core/file_utils.rs`, `src/bin/cli/commands.rs`
- `src/detectors/mod.rs`, `src/detectors/structure/mod.rs`
