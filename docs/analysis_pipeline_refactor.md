# Analysis Pipeline Refactor Blueprint

This note captures the concrete follow-ups from the latest architecture review. The items are ranked by expected impact and include the mechanical steps required to land them.

## 1. Pass-Oriented Pipeline

*Add a lightweight `AnalysisPass` trait and teach `AnalysisStages` to host a registry rather than concrete fields.*

### Steps
1. Define `AnalysisStageKind`, `StageResult`, and `AnalysisPassContext` so passes can emit typed results without downcasts.
2. Introduce the `AnalysisPass` trait (`fn name`, `fn kind`, `fn is_enabled`, `fn disabled_result`, `async fn run`).
3. Wrap existing stage structs (structure, coverage, complexity, refactoring, impact, LSH) inside `*_AnalysisPass` implementations.
4. Refactor `AnalysisStages::run_all_stages` to iterate over `passes: Vec<Box<dyn AnalysisPass>>` and compose a `StageResultsBundle` from the emitted `StageResult`s.
5. Keep compatibility helpers (`run_structure_analysis`, etc.) by delegating to the matching pass until downstream call sites are adjusted.
6. Update tests to exercise the pass registry (e.g., verifying disabled passes only emit default structs).

### Rationale
* Isolates hot path logic, unblocks per-pass configuration, and removes the God-object anti-pattern that currently exists in `pipeline_stages.rs`.

## 2. Streaming Token/Shingle Iterators for LSH

*Eliminate the heavyweight `Vec<String>` allocations in `detectors/lsh`.*

### Steps
1. Introduce `struct TokenStream<'a>` + `impl Iterator<Item = &'a str>` so tokenization becomes lazily evaluated.
2. Build `ShingleStream<'a, T>` over `TokenStream` to yield rolling windows without allocating intermediate `Vec`s.
3. Update `LshExtractor::create_shingles_internal` to accept the iterator pair, piping directly into the hashing/minhash stage.
4. Benchmark on the `datasets` fixtures to confirm the allocator pressure drop and adjust the default buffer sizes.

### Rationale
*Reduces memory bandwidth and lets us overlap shingling with hashing, which is dominated by vector allocation today.*

## 3. Semantic Symbol Table for Dependency Graphs

*Replace regex-based import resolution with a two-pass symbol table.*

### Steps
1. Create a `SymbolTableBuilder` pass that records `exports` per file (public functions, classes, types) via the existing AST service.
2. Extend `ProjectDependencyAnalysis` to accept the `SymbolTable` and resolve edges by symbol identity (file + export) instead of string equality.
3. Update graph-based detectors (`impact`, chokepoint computation) to consume the richer edges and surface precise refactoring hints (e.g., “move `User.save` from `user_repo.py`”).

### Rationale
*Removes false positives where identically named symbols in separate modules currently count as dependencies on each other.*

## 4. Dependency-Aware Cache Invalidation

*Keep the existing file-level cache but skip recomputation for unaffected dependents.*

### Steps
1. For each analyzed file, record an `ExportSignature` (hash of public API) alongside the file hash.
2. During incremental runs, compare both the file hash and the export signature—if the latter is unchanged, skip recomputing downstream coupling metrics.
3. Persist a dependency adjacency list in the cache so we can rapidly identify impacted files when an export signature changes.

### Rationale
*Lets CI runs short-circuit entire dependency subgraphs when edits do not change the public API surface.*

---

These items are tracked in decreasing order of payoff. The first two focus on raw performance, while the latter two improve analysis fidelity and developer ergonomics. When we pick up the work we should treat this document as the source of truth for acceptance criteria.
