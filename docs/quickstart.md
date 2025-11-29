# Quickstart

Follow these steps to run your first Valknut analysis and produce a shareable report.

## 1. Install

```bash
# from repo root
cargo install --path .
```

Alternatively run directly without installing:

```bash
cargo run --release -- analyze .
```

## 2. Create a config (optional)

```bash
cp valknut.yml.example valknut.yml
# tweak modules/languages/include patterns as needed
```

The CLI auto-discovers `.valknut.yml` / `.valknut.yaml` in the current directory, or you can pass `--config path/to/file`.

## 3. Analyze your codebase

```bash
valknut analyze . \
  --format html \
  --out .valknut/reports
```

This runs all analyses (complexity, structure, duplicates, refactoring, coverage) and writes an interactive `team_report.html`.

## 4. Common flags

- `--format json|jsonl|yaml|markdown|html|csv|sonar|ci-summary`
- `--no-coverage` or `--coverage-file coverage.lcov`
- `--profile fast|balanced|thorough|extreme`
- `--quality-gate` with `--min-health` / `--max-complexity` for CI exits

## 5. Open the report

```
open .valknut/reports/team_report.html   # macOS
xdg-open .valknut/reports/team_report.html   # Linux
```

You can also emit machine-friendly JSON/JSONL for further processing.
