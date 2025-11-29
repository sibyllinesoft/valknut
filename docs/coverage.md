# Coverage How‑To

Valknut ingests existing coverage to focus refactors and testing effort.

## Supported formats
- LCOV (`coverage.lcov`, `lcov.info`)
- Cobertura XML (`coverage.xml`, `cobertura.xml`)
- Istanbul/NYC JSON (`coverage-final.json`, `coverage.json`)
- Tarpaulin LCOV (`tarpaulin-output.info`)

## Discovery rules
Configured in `valknut.yml`:

```yaml
analysis:
  coverage:
    enabled: true
    auto_discover: true
    search_paths:
      - "./coverage/"
      - "./target/coverage/"
      - "./reports/"
    file_patterns:
      - "coverage.lcov"
      - "coverage.xml"
      - "lcov.info"
      - "**/coverage.xml"
    max_age_days: 7
```

You can bypass discovery with `--coverage-file path/to/report`.

## Cookbook by stack

- **Rust**: `cargo tarpaulin --out Lcov --output-dir coverage` → `coverage/lcov.info`.  
- **Python (pytest-cov)**: `pytest --cov --cov-report=lcov:coverage.lcov`.  
- **JavaScript/TypeScript (nyc/jest)**: `nyc --reporter=lcov npm test` or `jest --coverage` → `coverage/lcov.info`.  
- **Go**: `go test ./... -coverprofile=coverage.out && gocov convert coverage.out > coverage.lcov`.  
- **Java/JVM**: Jacoco → convert to Cobertura (`jacoco2cobertura`) or LCOV (`genhtml`/`lcov_cobertura`).  
- **C/C++ (gcovr)**: `gcovr -r . --xml -o coverage.xml` or `--lcov -o coverage.lcov`.

Place the output in any `search_paths` directory or pass it directly.

## What Valknut does with coverage
- Builds per-file coverage packs with uncovered spans (`coverage_packs` in JSON).  
- Computes `overall_coverage_percentage` and augments refactoring candidates with `coverage_percentage` where applicable.  
- Highlights high-impact gaps using complexity + fan-in to prioritize tests.

## Troubleshooting
- “No coverage files found”: ensure the file name matches `file_patterns` and isn’t older than `max_age_days`.  
- Mixed monorepo paths: add more `search_paths` (e.g., `packages/*/coverage`).  
- Format errors: run the generator’s “lcov” or “cobertura” output explicitly; compressed or HTML reports aren’t parsed.
