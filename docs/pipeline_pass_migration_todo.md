# Pipeline Pass Registry Migration TODO

Owner handoff notes for completing the pass-oriented pipeline refactor and removing legacy artifacts.

## Current State (as of commit in working tree)
- `AnalysisStages` now runs via a pass registry (`AnalysisPass` trait, `StageResult`) and exposes `pass_names()`.
- Public legacy per-stage helpers were removed; only private helpers remain for passes/tests.
- LSH shingling is streaming (TokenStream + rolling window) across cached and interned paths.
- LSH caches use deterministic FIFO eviction.
- Full test suite passes (`cargo test -q --locked --package valknut-rs --lib`).

## Remaining Work
1) **Result conversion & API surface**
   - Update `src/core/pipeline/result_conversions.rs` and any API layer structs to consume the pass registry outputs directly and to drop legacy fields that were tied to per-stage helpers.
   - Remove or adapt any conversions that expect `run_structure_analysis`/`run_complexity_analysis` public helpers.
   - Ensure `ComprehensiveAnalysisResult` wiring still intact after registry-first changes.

2) **Remove legacy fields from results**
   - Fields to delete end-to-end: `refactoring_candidates_by_file`, `unified_hierarchy`, `directory_health_tree`, and any other deprecated hierarchy outputs noted in `docs/analysis_normalization_todo.md`.
   - Clean up serde defaults and schema docs accordingly (`src/core/pipeline/pipeline_results.rs`, `result_types.rs`).

3) **Front-end / report pipeline updates**
   - ✅ Templates and JS now consume flattened `refactoring_candidates` plus `passes`; legacy `refactoring_candidates_by_file`, `unifiedHierarchy`, and directory trees are removed.
   - Updated files: 
     - `templates/dev/src/tree-component/CodeAnalysisTree.jsx`
     - `templates/dev/scripts/render-report.cjs`
     - `templates/partials/tree.hbs`
     - Bundled asset regenerated `templates/assets/react-tree-bundle.js`
   - Follow-up: refresh example HTML/SUMMARY partials if downstream screenshots rely on legacy keys.

4) **CLI output & API models**
   - Check `src/bin/cli/output.rs`, `src/api/results/models.rs`, `src/api/results/merge.rs` for references to removed fields; adjust to emit registry fields only.

5) **Docs**
   - README and `docs/ARCHITECTURE_DEEP_DIVE.md` should describe the pass registry as the sole pipeline interface; remove mention of per-stage helper calls.
   - Update `docs/analysis_pipeline_refactor.md` to mark completed items and add acceptance for template/report migration.

6) **Follow-up QA**
   - Pending: `cargo test -q --locked --package valknut-rs --lib` and front-end tests (`templates/dev/tests/*`).
   - Manual sanity: run CLI against a sample repo ensuring output renders without legacy fields.

## Notes / Risks
- Removing legacy fields will break consumers of the existing JSON/HTML reports; ensure downstream tooling is updated in the same change set.
- The UI tree currently tolerates both new and legacy structures; stripping legacy keys requires coordinated template/JS edits.
- No external callers to removed public helpers were found in `src/api`/`src/bin`, but double-check once result conversion changes land.

## Suggested Order of Ops
1. Remove legacy fields in result structs and conversions; adapt API/CLI emitters.
2. Update templates/React tree to the new shape; regenerate bundles if needed.
3. Docs refresh.
4. Final test pass.
