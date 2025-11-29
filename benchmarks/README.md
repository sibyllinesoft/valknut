# Valknut Benchmarks

Criterion-based micro/meso benchmarks for the Rust engine. Targets cover LSH, clone denoising, memory pools, and end-to-end pipeline performance.

## Running

```bash
cargo bench --features benchmarks
```

Notes:
- `harness = false` benches live in `benchmarks/src/performance.rs`; additional suites are in the same directory.
- The `benchmarks` feature gate keeps Criterion out of normal builds.
- Use `--profile profiling` if you want full debug symbols for profiling:
  ```bash
  cargo bench --features benchmarks --profile profiling
  ```

## Results

Benchmark output (Criterion reports) are written to `target/criterion/`. The `benchmarks/results/` directory is available if you want to persist or compare runs.
