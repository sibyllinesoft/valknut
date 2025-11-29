# Refactoring Playbook

Use Valknut findings as triggers for quick, low-risk improvements.

## High cyclomatic / cognitive complexity
**Symptoms:** CC/CogC above thresholds, “Complexity Hotspot” issues in reports.  
**Tactics:**  
- Add guard clauses to exit early.  
- Extract pure helpers; keep functions < ~40 lines where possible.  
- Flatten boolean chains (`match`/`switch` or lookup tables).  
- Prefer composition over long inheritance chains (TS/JS).  
**Patterns to apply:** Extract Method, Decompose Conditional, Replace Nested Conditionals with Guard Clauses.  
**Languages:**  
- Rust: use `match` for branching; split into smaller private fns; leverage enums for state.  
- Python/JS/TS: replace `if/elif` ladders with dict/Map dispatch; use early returns.

## Large functions / files
**Symptoms:** High LOC, MI < 70, structure packs suggest splits.  
**Tactics:**  
- Split by responsibility; move IO/parsing separate from business logic.  
- In Rust, new module or impl block per concept; in TS/JS, new file per concern.  
**Patterns:** Extract Class/Module, Introduce Parameter Object (when arg lists are long).

## Duplicate code (LSH/denoise hits)
**Symptoms:** Clone tab highlights pairs; refactoring issues mention “duplicates”.  
**Tactics:**  
- Extract shared helper; parameterize differences.  
- For tests, use fixtures/builders; for frontends, shared components/hooks.  
**Patterns:** Consolidate Duplicate Conditional Fragments, Template Method, DRY helpers.

## Coverage gaps
**Symptoms:** Coverage tab shows uncovered spans; `coverage_percentage` low.  
**Tactics:**  
- Write targeted unit tests around exported functions touched by gaps.  
- Prioritize gaps with high `cyclomatic_in_gap` or `fan_in_gap`.  
**Patterns:** Characterization tests before refactors; golden tests for parsers.

## Structural imbalance (architecture)
**Symptoms:** Structure detector suggests splits; directories exceed LOC/file counts.  
**Tactics:**  
- Enforce boundary directories (core/io/api).  
- Limit directory breadth; move heavy files into submodules.  
**Patterns:** Package by feature, not layer; Strangler for gradual moves.

## Dependency chokepoints / cycles
**Symptoms:** Impact analysis calls out high betweenness or cycles.  
**Tactics:**  
- Break cycles by inverting dependencies (interfaces/traits).  
- Move shared DTOs/types to dedicated module with no heavy deps.  
**Patterns:** Dependency Inversion, Stable Interfaces, Anti-corruption layers.

## How to use this with Valknut
1. Run `valknut analyze --format html --out .valknut/reports`.  
2. Open the HTML report: start with Complexity/Structure tabs, then Clones and Coverage.  
3. For each hotspot, pick a tactic above and write a small, test-backed change.  
4. Re-run `valknut analyze` and compare scores; iterate until CC/CogC/coverage improve.
