# Analysis Normalization TODO

- [x] Remove redundant hierarchy outputs (`refactoring_candidates_by_file`, `directory_health_tree`, `unified_hierarchy`) once all consumers finish migration.
- [x] Store refactoring candidates only in flat list (`refactoring_candidates`).
- [x] Propagate pass-oriented pipeline registry into engine/public API and delete legacy per-stage helpers.
- [ ] Benchmark streaming shingle changes vs cached Vec path on fixtures; tune shingle/window sizes.
- [ ] Extend streaming pipeline to any remaining LSH helpers (interned/cached already done).
- [x] Update CLI/markdown templates to consume normalized outputs only; drop legacy tree expectations.
