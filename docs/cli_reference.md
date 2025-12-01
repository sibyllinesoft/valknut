# CLI Reference

Authoritative summary of the current Rust CLI (`src/bin/cli/args.rs`). Use `valknut --help` and `valknut <cmd> --help` for the source of truth; this page mirrors those flags.

## Top-level commands

- `valknut analyze [PATHS...]` – full analysis pipeline (defaults to `.`).
- `valknut print-default-config` – dump built-in config to stdout.
- `valknut init-config [--output .valknut.yml] [--force]` – write a starter config file.
- `valknut validate-config --config <PATH> [--verbose]` – schema/semantic validation.
- `valknut list-languages` – show supported languages and parser status.
- `valknut doc-audit [--root .] [--strict] [--format text|json]` – standalone documentation/README audit.
- `valknut mcp-stdio [--config <PATH>]` – start the MCP server for editors/agents.
- `valknut mcp-manifest [--output manifest.json]` – emit MCP manifest JSON.

Global flags: `-v/--verbose`, `--survey`, `--survey-verbosity {low|medium|high|maximum}`.

## analyze command – core flags

- `--config <FILE>` – use explicit config (otherwise auto-discover).
- `--out <DIR>` (default `.valknut`) – report/output directory.
- `--format {jsonl,json,yaml,markdown,html,sonar,csv,ci-summary,pretty}`.
- `--quiet` – suppress console chatter (also implied by machine formats).
- `--profile {fast,balanced,thorough,extreme}` – speed/coverage presets.

### Quality gates (CI / fail builds)

Enable with `--quality-gate` or `--fail-on-issues`, then optionally:
- `--max-complexity <0-100>`
- `--min-health <0-100>` (mirrors min maintainability)
- `--min-doc-health <0-100>`
- `--max-debt <0-100>`
- `--min-maintainability <0-100>`
- `--max-issues <int>`
- `--max-critical <int>`
- `--max-high-priority <int>`

### Coverage

- `--no-coverage` – skip coverage analysis.
- `--coverage-file <PATH>` – explicit LCOV/Cobertura/etc.
- `--no-coverage-auto-discover` – disable searching for coverage files.
- `--coverage-max-age <days>` – discard stale coverage (0 = no limit).

### Clone / duplicate detection

Core: `--semantic-clones`, `--strict-dedupe`, `--denoise`, `--denoise-dry-run`, `--min-function-tokens <int>`, `--min-match-tokens <int>`, `--require-blocks <int>`, `--similarity <0-1>`.

Advanced (rare): `--no-auto`, `--loose-sweep`, `--rarity-weighting`, `--structural-validation`, `--apted-verify` / `--no-apted-verify`, `--apted-max-nodes <int>`, `--apted-max-pairs <int>`, `--live-reach-boost`, `--ast-weight <0-1>`, `--pdg-weight <0-1>`, `--emb-weight <0-1>`, `--io-mismatch-penalty <0-1>`, `--quality-target <0-1>`, `--sample-size <int>`, `--min-saved-tokens <int>`, `--min-rarity-gain <float>`.

### Analysis toggles

`--no-complexity`, `--no-structure`, `--no-refactoring`, `--no-impact`, `--no-lsh`.

### AI features

`--oracle` – enable Gemini-powered refactoring oracle (requires `GEMINI_API_KEY`).  
`--oracle-max-tokens <int>` – cap token budget.

## doc-audit command – key flags

- `--root <PATH>` (default `.`)
- `--complexity-threshold <int>` (defaults come from `doc_audit`)
- `--max-readme-commits <int>`
- `--strict` – non-zero exit on findings
- `--format {text,json}`
- `--ignore-dir <NAME>` (repeatable), `--ignore-suffix <SUFFIX>`, `--ignore <GLOB>`
- `--config <FILE>` – optional doc-audit YAML

## Quick recipes

- CI summary: `valknut analyze --quality-gate --format ci-summary --out .valknut ./src`
- HTML for stakeholders: `valknut analyze --format html --out reports ./src`
- Fast local run: `valknut analyze --profile fast --quiet ./src`
- Strict clones with coverage: `valknut analyze --strict-dedupe --denoise --coverage-file coverage/lcov.info ./`

See `docs/CONFIG_GUIDE.md` and `docs/QUALITY_GATES_GUIDE.md` for deeper config semantics and examples.
