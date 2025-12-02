# Valknut – Fast, Multi-Layer Code Intelligence for Real Teams

Valknut is a Rust-native analysis platform that combines structural heuristics, AST-driven complexity metrics, documentation audits, and optional AI guidance. The CLI ships with CI-friendly output, a documentation linter, MCP endpoints for IDE automation, and optional refactoring oracle.

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Docs & Demo
- Docs site: https://valknut.sibylline.dev
- Live report snapshot: https://valknut.sibylline.dev/report-dev.html

## At a Glance
- **Comprehensive analysis pipeline** – structure, complexity, dependency graph, coverage, LSH clone detection, refactoring scoring, and health metrics driven by `AnalysisPipeline`.
- **Documentation awareness** – the bundled `doc-audit` command finds missing/dated READMEs, TODO clusters, and style regressions using the `crates/doc_audit` crate.
- **AI & MCP integration** – run `valknut mcp-stdio` to expose a Model Context Protocol server or enable the Gemini-powered refactoring oracle with `--oracle`.
- **High-performance internals** – arena allocation, shared AST caches, SIMD-accelerated similarity, and git-aware file discovery keep large repos manageable.
- **Battle-tested reports** – export JSONL/JSON/YAML/CVS/Markdown/HTML/Sonar/CI-summary formats plus colorized console summaries.

## Supported Languages (AST-level)
| Language | Status | Notes |
| --- | --- | --- |
| Python | ✅ Full support | Tree-sitter Python with structure/complexity/refactoring detectors |
| TypeScript / JavaScript | ✅ Full support | Handles `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` |
| Rust | ✅ Full support | Ownership-aware complexity & dependency graphs |
| Go | 🚧 Beta | AST parsing works; recommendations still limited |

> Valknut currently exposes only these adapters in `src/lang/registry.rs`. Other extensions will be skipped unless/until dedicated adapters are implemented.

## Commands at a Glance
| Command | Purpose |
| --- | --- |
| `valknut analyze [PATH]` | Run the full analysis pipeline with selectable profiles and output formats |
| `valknut doc-audit --root REPO` | Audit READMEs, TODO hot-spots, and stale docs using the `doc_audit` crate |
| `valknut list-languages` | Display the runtime language matrix (driven by the actual adapters) |
| `valknut init-config` / `print-default-config` | Scaffold or inspect `valknut.yml` |
| `valknut validate-config --config valknut.yml` | Sanity-check custom configuration files |
| `valknut mcp-stdio` / `mcp-manifest` | Launch the MCP server or emit a manifest for IDE agents |

## Installation
### Homebrew (macOS)
```bash
brew tap sibyllinesoft/valknut
brew install valknut
```

### Cargo (cross-platform)
```bash
cargo install valknut-rs
```

### Build from Source
```bash
git clone https://github.com/sibyllinesoft/valknut
cd valknut
cargo build --release
```

## Quickstart
```bash
# Fast scan with JSONL output (default profile)
valknut analyze ./src --format jsonl

# HTML + Markdown bundle for stakeholders
valknut analyze ./ --format html --profile thorough

# Documentation audit with strict exit codes
valknut doc-audit --root . --strict

# List the languages compiled into your build
valknut list-languages
```

### Profiles & Flags
- `--profile fast|balanced|thorough|extreme` selects how many detectors and optimizations run.
- `--no-structure`, `--no-impact`, `--no-lsh`, etc., mirror `analysis.modules.*` toggles in `valknut.yml`.
- Clone detection controls live under `--semantic-clones`, `--denoise`, `--min-function-tokens`, etc.

## Core Capabilities
**Structure Analysis** – deterministic directory/file re-organization packs (`src/detectors/structure`) surface imbalance, whale files, and recommended splits.

**Complexity Intelligence** – AST-backed cyclomatic/cognitive metrics and severity classification per entity (`src/detectors/complexity`).

**Dependency & Impact Analysis** – `ProjectDependencyAnalysis` builds call graphs, detects cycles, and feeds choke-point scoring plus similarity cliques.

**Clone Detection (opt-in)** – locality-sensitive hashing with optional denoising/simd speedups for semantic clone clusters.

**Coverage Awareness** – auto-discover or pin coverage files, surface gap summaries, and include them in health metrics.

**Refactoring Scoring** – aggregated feature vectors drive health, maintainability, and technical-debt indices for gating.

## Quality Gates & CI
GitHub Actions example:
```yaml
name: Valknut Quality Gate
on: [push, pull_request]

jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo install valknut-rs
      - run: |
          valknut analyze ./src \
            --format html \
            --out quality-reports \
            --quality-gate \
            --max-complexity 70 \
            --min-health 65
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: quality-reports
          path: quality-reports
```
Quality gates can also be expressed in config (`analysis.quality`) or via CLI flags (`--max-debt`, `--max-issues`, `--max-critical`, etc.).

## Documentation Audit
The `doc-audit` command walks the repo, scores directory complexity, and tracks README freshness:
```bash
valknut doc-audit --root . --complexity-threshold 10 --max-readme-commits 8 --strict
```
Use `--ignore-dir` / `--ignore-suffix` to skip generated assets. The audit exits non-zero in `--strict` mode when gaps exist, making it ideal for CI.

## AI Oracle & MCP
- **Refactoring Oracle**: `valknut analyze ... --oracle` streams the analysis summary plus curated code bundles to Gemini 2.5 Pro. Set `GEMINI_API_KEY` (and optionally `--oracle-max-tokens`) before enabling this opt-in path.
- **Model Context Protocol**: `valknut mcp-stdio` exposes the analyze/list/gate abilities to IDE agents. Use `valknut mcp-manifest --output manifest.json` to publish the schema from `src/bin/cli/commands.rs`.

## Configuration & Layering
- Run `valknut init-config` to generate `.valknut.yml` (see `valknut.yml.example` for every toggle).
- CLI → API → pipeline config layers are merged via `src/bin/cli/config_layer.rs`. Settings such as coverage search paths, structure thresholds, or LSH tuning can live in config files, environment variables, or direct flags.
- Profiles, module toggles, and quality gates can be version-controlled to keep CI deterministic.

## Output Formats & Reports
Select via `--format`:
- `jsonl`, `json`, `yaml` – machine-friendly ingestion.
- `markdown`, `html`, `pretty` – human-friendly reports powered by `src/io/reports` handlebars templates.
- `csv` – spreadsheet-ready metrics.
- `sonar` – SonarQube compatibility.
- `ci-summary` – concise JSON for bots.

## Development
```bash
cargo fmt && cargo clippy
cargo test
./scripts/install_parsers.sh  # install/update tree-sitter grammars
```
Helpful references:
- `docs/CLI_USAGE.md` – CLI walkthroughs.
- `docs/ARCHITECTURE_DEEP_DIVE.md` – November 2025 architectural analysis and modernization plan.
- `docs/CONFIG_GUIDE.md` / `docs/QUALITY_GATES_GUIDE.md` – configuration details.

## License
MIT License – see [LICENSE](LICENSE).
