# CI/CD Integration Examples for Valknut

This directory contains example CI/CD pipeline configurations for integrating Valknut code quality analysis into your development workflow.

## 🚀 Quick Start

### Basic Quality Gate Command
```bash
./valknut analyze . \
  --format ci-summary \
  --quality-gate \
  --max-issues 5 \
  --min-health 70 \
  --max-complexity 80 \
  --quiet
```

This command will:
- ✅ Analyze your codebase for quality issues
- 📊 Generate a CI-friendly JSON summary
- ⚠️ Fail with exit code 1 if quality gates are not met
- 🔇 Run in quiet mode (minimal output)

## 📋 Available CI/CD Examples

### 1. [GitHub Actions](github-actions.yml)
- **Features**: PR comments, artifact uploads, quality gate enforcement
- **Triggers**: Pull requests and pushes to main
- **Outputs**: CI summary JSON, detailed reports, PR comments

### 2. [GitLab CI](gitlab-ci.yml)
- **Features**: Merge request widgets, code quality reports, artifacts
- **Triggers**: Merge requests and main branch
- **Outputs**: GitLab-compatible quality reports, detailed HTML reports

### 3. [Azure Pipelines](azure-pipelines.yml)
- **Features**: Test result integration, build artifacts, pipeline variables
- **Triggers**: PRs and main branch pushes
- **Outputs**: Azure DevOps test results, analysis artifacts

### 4. [Jenkins Pipeline](jenkins.groovy)
- **Features**: HTML report publishing, build status updates, notifications
- **Triggers**: All branches (configurable)
- **Outputs**: Archived artifacts, HTML reports, build summaries

## ⚙️ Configuration Options

### Quality Gate Parameters
| Parameter | Description | Default | Example |
|-----------|-------------|---------|---------|
| `--max-issues` | Maximum allowed issues | 10 | `--max-issues 5` |
| `--min-health` | Minimum health score (0-100) | 60 | `--min-health 70` |
| `--max-complexity` | Maximum complexity score | 75 | `--max-complexity 80` |
| `--min-maintainability` | Minimum maintainability score | 50 | `--min-maintainability 60` |
| `--max-critical` | Maximum critical issues | 0 | `--max-critical 1` |
| `--max-high-priority` | Maximum high-priority issues | 3 | `--max-high-priority 2` |

### Output Formats
| Format | Description | Use Case |
|--------|-------------|----------|
| `ci-summary` | Concise JSON for CI/CD | Automated pipelines |
| `jsonl` | Line-delimited JSON | Full analysis data |
| `html` | Interactive HTML report | Human review |
| `sonar` | SonarQube compatible | SonarQube integration |
| `csv` | Spreadsheet format | Data analysis |

## 📊 CI Summary Output Structure

The `--format ci-summary` generates a structured JSON file perfect for CI/CD consumption:

```json
{
  "status": "issues_found",
  "summary": {
    "total_files": 1,
    "total_issues": 3,
    "critical_issues": 1,
    "high_priority_issues": 2,
    "languages": ["Python", "JavaScript"]
  },
  "metrics": {
    "overall_health_score": 73.1,
    "complexity_score": 28.8,
    "maintainability_score": 53.8,
    "technical_debt_ratio": 36.5,
    "average_cyclomatic_complexity": 8.0,
    "average_cognitive_complexity": 6.4
  },
  "quality_gates": {
    "health_score_threshold": 70.0,
    "complexity_threshold": 75.0,
    "max_issues_threshold": 5,
    "recommendations": [
      "Address high-priority issues first",
      "Focus on reducing complexity in critical files",
      "Improve maintainability through refactoring"
    ]
  },
  "timestamp": "2025-09-08T04:32:37Z",
  "analysis_id": "c0c02bc7-12b3-4207-bd28-f3ecd556c4c0"
}
```

## 🔧 Integration Patterns

### Pattern 1: Pull Request Quality Gates
```yaml
# Block merging if quality gates fail
on: pull_request
steps:
  - run: valknut analyze --quality-gate --max-issues 0
  # This will fail the PR if any issues exist
```

### Pattern 2: Trend Monitoring
```yaml
# Monitor quality trends over time
on: push
steps:
  - run: valknut analyze --format ci-summary
  - name: Store metrics
    run: |
      # Store metrics in time-series database
      # Track health score trends
```

### Pattern 3: Conditional Enforcement
```yaml
# Stricter rules for production branches
steps:
  - name: Quality Gate
    run: |
      if [[ "$GITHUB_REF" == "refs/heads/main" ]]; then
        valknut analyze --quality-gate --max-issues 0 --min-health 90
      else
        valknut analyze --quality-gate --max-issues 10 --min-health 60
      fi
```

### Pattern 4: Multi-Stage Analysis
```yaml
# Different analysis depth by branch
stages:
  - name: Quick Check
    run: valknut analyze --quality-gate --quiet
  - name: Detailed Analysis (main only)
    if: branch == 'main'
    run: valknut analyze --format html --out reports/
```

## 🛠️ Customization

### Custom Thresholds
Each project may need different quality thresholds. Consider:

- **Strict**: `--max-issues 0 --min-health 90 --max-complexity 60`
- **Moderate**: `--max-issues 5 --min-health 70 --max-complexity 75`
- **Relaxed**: `--max-issues 15 --min-health 50 --max-complexity 85`

### Incremental Analysis
For large codebases, consider analyzing only changed files:

```bash
# Get changed files from git
CHANGED_FILES=$(git diff --name-only HEAD~1)
if [ ! -z "$CHANGED_FILES" ]; then
  ./valknut analyze $CHANGED_FILES --quality-gate
fi
```

## 📈 Best Practices

### 1. **Fail Fast**
- Use quality gates on pull requests
- Set appropriate thresholds for your team
- Provide clear feedback to developers

### 2. **Progressive Enhancement**
- Start with relaxed thresholds
- Gradually tighten them as code quality improves
- Monitor trends, not just absolute values

### 3. **Context-Aware Rules**
- Different rules for different branches
- More lenient for experimental branches
- Strict enforcement for production releases

### 4. **Developer Experience**
- Generate actionable reports
- Provide clear error messages
- Include remediation guidance

### 5. **Automation**
- Archive reports for trend analysis
- Send notifications on quality degradation
- Integrate with code review tools

## 🔍 Troubleshooting

### Common Issues

1. **Quality gate always passes**
   - Check if analysis is actually running (`--quiet` hides output)
   - Verify thresholds are appropriate for your codebase
   - Ensure analysis finds files to analyze

2. **Pipeline fails unexpectedly**
   - Check Valknut binary permissions (`chmod +x valknut`)
   - Verify output directory exists
   - Check for sufficient disk space

3. **No analysis results**
   - Ensure target directory contains supported file types
   - Check file permissions and accessibility
   - Verify working directory in CI context

### Debug Commands
```bash
# Check what files Valknut will analyze
./valknut analyze . --format pretty

# Run with verbose output
./valknut analyze . --verbose

# Test quality gates with current settings
./valknut analyze . --quality-gate --max-issues 999 --quiet
```

## 📞 Support

For more information and support:
- 📖 [Documentation](../README.md)
- 🐛 [Issues](https://github.com/your-repo/valknut/issues)
- 💬 [Discussions](https://github.com/your-repo/valknut/discussions)