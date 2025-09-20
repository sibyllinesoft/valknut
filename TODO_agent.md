# Valknut Cleanup TODOs

## Phase 1 – Critical Architectural Cleanup
- [ ] Unify the configuration system around `api::config_types::AnalysisConfig`
  - [x] Remove `core::config` duplication and legacy conversions
  - [x] Update detectors and pipeline stages to consume the unified config
  - [x] Excise unused `names_analysis` knobs from configs and docs
- [ ] Unify the analysis results model
  - [ ] Remove the translation layer in `src/api/results/merge.rs`
  - [ ] Split `src/api/results/models.rs` into focused modules under `src/api/results/`
- [ ] Unify clone/duplication detection
  - [ ] Remove the token fingerprint logic in `src/detectors/refactoring.rs`
  - [ ] Route refactoring recommendations through the LSH extractor
  - [ ] Factor `src/detectors/lsh/mod.rs` into submodules for maintainability
- [ ] Ensure the MCP server respects user configuration (wire loaded config through `mcp_stdio_command`)

## Phase 2 – Refactoring and Implementation Fixes
- [ ] Break down monolithic modules
  - [ ] Extract CLI command handlers from `src/bin/cli/commands.rs`
  - [ ] Decompose `src/detectors/structure/directory.rs`
  - [ ] Split `src/io/cache.rs` into cache primitives and stop-motif specialisations
- [ ] Fix stubbed/inefficient integrations
  - [ ] Refactor `execute_refactoring_suggestions` in `src/bin/mcp/tools.rs` for targeted analysis
  - [ ] Update the VS Code extension export flow to shell out to the Valknut CLI
- [ ] Frontend and asset cleanup
  - [ ] Standardise on the React tree component and remove vanilla JS fallbacks
  - [ ] Move React source from `templates/assets/src` into a dedicated `frontend/` workspace
  - [ ] Standardise the asset toolchain (choose Bun vs Webpack and consolidate `package.json`)
  - [ ] Stop tracking generated bundles and expand `.gitignore`

## Phase 3 – Polish and Final Audit
- [ ] Rewrite `benches/memory_pool_benchmark.rs` to measure allocation savings
- [ ] Align real CLI output with the design in `examples/cli_output_demo.py`
- [ ] Consolidate docs, archive outdated guides, and refresh the README
- [ ] Backfill targeted unit tests for structure and coverage metrics
- [ ] Document the updated pack outputs and configuration knobs
- [ ] Run the full QA suite (`scripts/dev-lint.sh`, `scripts/dev-test.sh`, `scripts/benchmark.sh`)

## Supporting Backlog
- [ ] Flesh out coverage gap analysis in `src/detectors/coverage/mod.rs`
- [ ] Wire clone/coverage data through to the frontend `CodeAnalysisTree`
- [ ] Deprecate legacy report data formats and tighten frontend error handling
- [ ] Add regression tests for dependency `select_target` heuristics
- [ ] Organize `docs/` with an index and tidy archived content
- [ ] Introduce a task runner (`Makefile` or `justfile`) consolidating scripts
- [ ] Audit library code for `unwrap`/`expect` and replace with structured errors

## Completed Milestones
- [x] Replace placeholder logic in `src/detectors/structure/file.rs` for split value and effort
- [x] Implement robust coverage parsers in `src/detectors/coverage/parsers.rs`
- [x] Implement end-to-end clone pair harvesting in `run_lsh_analysis`
- [x] Rebuild coverage gap analysis to prioritise uncovered high-complexity code segments
- [x] Collapse `AnalysisResults` and `ComprehensiveAnalysisResult` into a single model
- [x] Update pipeline builders to emit the unified result type directly
- [x] Cache project-wide import analysis for structure detector performance
