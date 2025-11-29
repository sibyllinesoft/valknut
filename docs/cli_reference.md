# CLI Reference

This page lists the primary commands and high-signal flags exposed by the `valknut` binary. All options come from `src/bin/cli/args.rs`.

## Top-level

- `valknut analyze [PATHS...]` — run the full analysis pipeline (defaults to `.`).
- `valknut print-default-config` — print the built-in config to stdout.
- `valknut init-config [--output .valknut.yml] [--force]` — write a starter config file.
- `valknut validate-config --config path.yml [--verbose]` — schema/semantic validation.
- `valknut list-languages` — show supported languages and parser status.
- `valknut doc-audit [--config]` — documentation coverage/readme audit.
- `valknut mcp-stdio [--config]` — start the MCP server for editor agents.
- `valknut mcp-manifest [--output manifest.json]` — emit MCP manifest JSON.

Global flags:

- `-v/--verbose` — debug logging.
- `--survey` and `--survey-verbosity {low,medium,high,maximum}` — opt-in analytics prompts.

## analyze command (core flags)

- `--config <FILE>` — override config discovery.
- `--out <DIR>` (default `.valknut`) — where reports/results land.
- `--format {jsonl,json,yaml,markdown,html,sonar,csv,ci-summary,pretty}` — output shape.
- `--quiet` — minimal console output.
- `--profile {fast,balanced,thorough,extreme}` — speed vs depth presets.

### Quality gates (fail builds)

- `--quality-gate` or `--fail-on-issues`
- `--max-complexity <0-100>`
- `--min-health <0-100>`
- `--min-doc-health <0-100>`

### Coverage

- `--no-coverage` — skip coverage analysis.
- `--coverage-file <PATH>` — explicit LCOV/Cobertura/etc.
- `--coverage-search-path <PATH>` (repeatable) — extra discovery roots.
- `--coverage-max-age <days>`

### Clone / duplicate analysis

- `--no-duplicates` — disable.
- `--shingle-k <int>` — shingle size for LSH.
- `--min-match-tokens <int>` / `--min-ast-nodes <int>`
- `--similarity-threshold <0-1>` — LSH similarity cutoff.

### Analysis toggles

- `--no-complexity`, `--no-structure`, `--no-refactoring`, `--no-impact`, `--no-coverage`, `--no-graph`
- `--languages <list>` — limit languages (comma-separated).
- `--include <glob>` / `--exclude <glob>` — file filters.
- `--max-files <int>` — cap scanned files.

### AI features

- `--no-oracle` — skip AI refactoring hints.
- `--oracle-max-tokens <int>` — cap token budget for hints.

## Output formats (what you get)

- `jsonl` — line-delimited events, good for streaming/logs.
- `json` / `yaml` — structured snapshot of full results.
- `markdown` — team-friendly summary (`team_report.md`).
- `html` — interactive report with treemap and drilldowns.
- `csv` — tabular issues for spreadsheets.
- `sonar` — SonarQube-compatible issues JSON.
- `ci-summary` — small JSON for CI status checks.
- `pretty` — colorized console dump.

## Tips

- Use `valknut analyze --format html --out .valknut/reports` for stakeholder-friendly output.
- In CI, pair `--quality-gate --min-health 80 --max-complexity 75` with `--format ci-summary` and parse the small JSON.
- For monorepos, run multiple `analyze` invocations per package and merge reports downstream. +#+
