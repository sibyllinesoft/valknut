# Metrics Reference

How Valknut scores your code and where those numbers appear in reports.

## Core metrics

- **Cyclomatic Complexity (CC)**  
  Counts distinct decision points (+1 for function entry). Implemented in `src/detectors/complexity.rs` using branch/loop/logical operators. Reference: McCabe 1976.

- **Cognitive Complexity (CogC)**  
  Sonar-style: penalizes nesting and boolean chains more than raw branches. Implemented alongside CC with nesting levels. Reference: SonarSource whitepaper (2016).

- **Maintainability Index (MI)**  
  Normalized 0‑100 score combining Halstead volume, CC, and LOC (variant of SEI/VS). Higher is better. MI is computed in `core::ast_service`.

- **Halstead** (volume/effort/bugs)  
  Token-level counts reported per entity; used as inputs for MI and debt heuristics.

- **Coverage %**  
  From LCOV/Cobertura/Istanbul ingestion; surfaced per file and as `overall_coverage_percentage` in pipeline results.

## Where to find them in reports

- **HTML/Markdown**: Complexity and Structure tabs list CC/CogC/MI; Coverage tab shows uncovered spans and estimated gains.  
- **JSON/JSONL**: look under `results.complexity.detailed_results[*].metrics.{cyclomatic_complexity,cognitive_complexity,maintainability_index}` and `coverage.overall_coverage_percentage`.  
- **CSV/Sonar**: CC/CogC mapped to issue descriptions for hotspots.

## Default thresholds

Configured in `valknut.yml`:

```yaml
analysis:
  languages:
    complexity_thresholds:
      rust: 15
      python: 10
      javascript: 10
      typescript: 10
      go: 12
analysis:
  quality:
    confidence_threshold: 0.7
scoring:
  weights:
    complexity: 1.0
    coverage: 0.7
```

You can override per language and adjust quality gates with `--max-complexity`, `--min-health`, and `--min-maintainability`.

## Interpreting scores

- CC > threshold: risk of branching bugs and test gaps; prefer extraction and guard clauses.  
- CogC high with moderate CC: usually deep nesting/boolean chains—flatten logic first.  
- MI < 70: treat as “needs refactor soon”; < 50: critical.  
- Coverage < 70% on critical files: address gaps with targeted tests (see Coverage How‑To).

## Recommended reading

- McCabe, *A Complexity Measure* (1976).  
- SonarSource, *Cognitive Complexity* whitepaper (2016).  
- SEI Maintainability Index documentation (Visual Studio variant).
