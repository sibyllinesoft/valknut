# Valknut Quality Gates Guide

This guide demonstrates how to use Valknut's quality gate features for CI/CD integration and code quality enforcement.

## CLI Flags (All Working ✅)

All quality gate CLI flags are **fully implemented and working**:

```bash
# Enable quality gate mode
valknut analyze --quality-gate .

# Set custom complexity threshold (0-100, lower is better)  
valknut analyze --quality-gate --max-complexity 50 .

# Set minimum health score (0-100, higher is better)
valknut analyze --quality-gate --min-health 70 .

# Set maximum technical debt ratio (0-100, lower is better)
valknut analyze --quality-gate --max-debt 25 .

# Set minimum maintainability index (0-100, higher is better)
valknut analyze --quality-gate --min-maintainability 30 .

# Set maximum total issues count
valknut analyze --quality-gate --max-issues 25 .

# Set maximum critical issues count
valknut analyze --quality-gate --max-critical 0 .

# Set maximum high-priority issues count  
valknut analyze --quality-gate --max-high-priority 3 .

# Shorthand for quality gate mode
valknut analyze --fail-on-issues .
```

## Configuration File Support (✅ JSON & YAML)

Both JSON and YAML configuration files are supported:

### Using JSON Configuration

```bash
valknut analyze --config .valknut.yml .
```

### Using YAML Configuration  

```bash
valknut analyze --config .valknut.yml .
```

### Sample .valknut.json

```json
{
  "structure": {
    "enable_branch_packs": true,
    "enable_file_split_packs": true,
    "top_packs": 20
  },
  "fsdir": {
    "max_files_per_dir": 25,
    "max_subdirs_per_dir": 10,
    "max_dir_loc": 2000,
    "min_branch_recommendation_gain": 0.15,
    "min_files_for_split": 5,
    "target_loc_per_subdir": 1000
  },
  "fsfile": {
    "huge_loc": 800,
    "huge_bytes": 128000,
    "min_split_loc": 200,
    "min_entities_per_split": 3
  },
  "partitioning": {
    "balance_tolerance": 0.25,
    "max_clusters": 4,
    "min_clusters": 2,
    "naming_fallbacks": ["core", "io", "api", "util"]
  }
}
```

## Quality Gate Defaults

| Setting | Default Value | Description |
|---------|---------------|-------------|
| max_complexity | 75.0 | Maximum complexity score (lower is better) |  
| min_health | 60.0 | Minimum health score (higher is better) |
| max_debt | 30.0 | Maximum technical debt ratio (lower is better) |
| min_maintainability | 20.0 | Minimum maintainability index (higher is better) |
| max_issues | 50 | Maximum total issues count |
| max_critical | 0 | Maximum critical issues count |  
| max_high_priority | 5 | Maximum high-priority issues count |

## Example CI/CD Integration

### GitHub Actions

```yaml
name: Code Quality Gate
on: [push, pull_request]

jobs:
  quality-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Build Valknut
        run: cargo build --bin valknut --release
      - name: Run Quality Gate
        run: |
          ./target/release/valknut analyze \
            --quality-gate \
            --max-complexity 50 \
            --min-health 70 \
            --max-debt 20 \
            --max-issues 25 \
            --max-critical 0 \
            .
```

### CLI Exit Codes

- **Exit Code 0**: All quality gates passed ✅
- **Exit Code 1**: One or more quality gates failed ❌

## Quality Gate Output Example

When quality gates fail, you'll see detailed output like:

```
❌ Quality Gate Failed
Quality Score: 3

🚫 BLOCKER Issues:
  • Overall Health Score: 36.0 (threshold: 60.0)
    Health score (36.0) is below minimum required (60.0)

🔴 CRITICAL Issues:
  • Total Issues Count: 523.0 (threshold: 50.0)
    Total issues (523) exceeds maximum allowed (50)
  • Complexity Score: 100.0 (threshold: 75.0)  
    Complexity score (100.0) exceeds maximum allowed (75.0)
  • Technical Debt Ratio: 80.5 (threshold: 30.0)
    Technical debt ratio (80.5%) exceeds maximum allowed (30.0%)
```

## Troubleshooting

### Issue: "No such option: --quality-gate"
**Status**: ❌ This should not occur - all flags are implemented and working.

**Solution**: 
1. Verify you're using the correct binary: `cargo run --bin valknut -- analyze --help`
2. Check that the help output shows the quality gate options
3. Ensure you're running the latest build: `cargo build --bin valknut`

### Issue: Configuration file not found
**Solution**:
```bash
# Check file exists
ls -la .valknut.json

# Verify JSON syntax  
cat .valknut.json | jq .

# Use absolute path if needed
valknut analyze --config /path/to/.valknut.json .
```

## Advanced Usage

### Combining CLI and Config
CLI flags override configuration file settings:

```bash
# Uses .valknut.json for structure settings, CLI for quality gates
valknut analyze \
  --config .valknut.json \
  --quality-gate \
  --max-complexity 40 \
  .
```

### Different Output Formats with Quality Gates

```bash  
# JSON output for automated processing
valknut analyze --quality-gate --format json .

# HTML report with quality gate results
valknut analyze --quality-gate --format html --out reports/ .

# CI-friendly summary format
valknut analyze --quality-gate --format ci-summary .
```