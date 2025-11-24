# Doc Health Integration TODO

Owner: tooling / pipeline
Goal: make documentation gaps a first-class health signal aggregated bottom-up (entity → file → directory → project) with configurable eligibility thresholds.

## Functional Tasks
- [x] Wire `doc_audit` (or equivalent doc-gap scanner) into the main analysis pipeline so per-file doc gap stats are emitted alongside complexity/refactoring data.
- [x] Define `DocHealthThresholds` config (defaults: min_fn_nodes=5, min_file_nodes=50, min_files_per_dir=5) and plumb into CLI/config files.
- [ ] Per-file stats: capture `doc_health_score`, `eligible_doc_items`, `doc_gap_count`, and expose them to scoring/aggregation.
- [ ] Directory aggregation: compute doc coverage/health from child files + subdirectories; include doc counts in directory issue totals.
- [x] Project aggregation: add `doc_health_score` to `HealthMetrics` and include it in the overall blend.
- [ ] Issue model: count doc gaps as standard issues (severity tunable) so quality gates and issue-rate penalties see them.
- [ ] Update `DirectoryHealthTree` schema and JSON outputs to surface doc health fields (keep backward compatibility where possible).
- [ ] Add UI/report rendering for doc health (tree, summary, quality gate messaging).

## Normalization & Scoring
- [ ] Implement doc coverage normalization: file coverage = (eligible_items - gaps) / eligible_items; directory/project = weighted mean (LOC-weighted) with eligibility thresholds.
- [ ] Add logistic/soft-hard mappings for:
  - LOC per file (log scale 150–600 window)
  - Files per directory (logistic, soft 4, hard ~10)
  - Functions per file (logistic, soft 8, hard ~20)
  - Classes per file (soft 1, hard 3)
  - Maintain existing complexity mappings; plug doc health into the blended score.

## Config & CLI
- [ ] Extend CLI/config schema with doc thresholds and optional `--min-doc-health` gate.
- [ ] Ensure defaults are backward compatible; gating remains off unless configured.

## Tests
- [ ] Unit tests for doc normalization (eligible vs ineligible cases, thresholds respected).
- [ ] Integration test: pipeline run producing doc health in results JSON and respecting directory eligibility threshold.
- [ ] Quality gate test: fail when doc health below threshold; pass otherwise.

## Migration / Compatibility
- [ ] Keep existing fields; add new ones rather than removing to avoid breaking consumers.
- [ ] Update sample reports/templates to include doc health (with fallback when missing).
