# Team Reports and Integration Guide

Valknut now supports professional report formats designed for team sharing, stakeholder presentations, and integration with popular development tools.

## Overview

The new team reporting system generates structured, visually appealing reports in multiple formats:

- **📄 Markdown Tables** - Structured markdown with visual indicators for team reviews
- **🌐 Professional HTML** - Interactive reports for presentations and sharing
- **🔧 SonarQube JSON** - Direct integration with SonarQube for issue tracking
- **📊 CSV Export** - Data format for spreadsheets and team dashboards

## Quick Start

### Basic Usage

```bash
# Generate HTML report for team presentations
valknut analyze --format html --out team-report my-project/

# Create markdown report for code reviews
valknut analyze --format markdown --out reports/ src/

# Export SonarQube format for CI/CD integration
valknut analyze --format sonar --out build/quality/ backend/

# Generate CSV for spreadsheet analysis
valknut analyze --format csv --out metrics/ frontend/
```

### Command Line Options

```bash
valknut analyze [OPTIONS] PATHS...

Options:
  -c, --config PATH    Configuration file path
  -o, --out PATH       Output directory (default: out)
  --format FORMAT      Output format: jsonl|json|markdown|html|sonar|csv
  --help              Show help message

Examples:
  valknut analyze --format html --out reports/ src/
  valknut analyze --format sonar --config quality.yaml api/
```

## Report Formats

### 📄 Markdown Reports (`--format markdown`)

Generates structured markdown tables perfect for:
- **Code review discussions**
- **GitHub/GitLab issue tracking**  
- **Team documentation**
- **README quality summaries**

**Generated Files:**
- `team_report.md` - Comprehensive team report with tables and visual indicators
- `summary.md` - Legacy summary (backward compatibility)
- `refactoring_guide.md` - Legacy detailed guide (backward compatibility)

**Features:**
- Executive summary with health score
- Language breakdown with status indicators (✅ ⚠️ ❌)
- Critical issues table with severity badges
- Prioritized refactoring recommendations
- Technical debt metrics with targets
- Next steps and sprint planning guidance

**Example Output:**
```markdown
# 📊 Code Quality Report: MyProject

**Generated:** 2024-01-15 10:30:45  
**Overall Health Score:** 🟡 72/100

## 📈 Language Breakdown

| Language | Files | Entities | Avg Score | Status |
|----------|-------|----------|-----------|---------|
| Python   | 45    | 234      | 0.42      | ✅      |
| TypeScript | 23  | 156      | 0.68      | ⚠️      |

## 🚨 Critical Issues Requiring Attention

| Entity | Severity | Score | Primary Issues |
|--------|----------|-------|----------------|
| `complex_algorithm` | 🔴 CRITICAL | 0.891 | High cyclomatic complexity (15.2) |
```

### 🌐 HTML Reports (`--format html`)

Professional, interactive HTML reports ideal for:
- **Stakeholder presentations**
- **Management dashboards** 
- **Team meetings**
- **Mobile viewing**

**Generated Files:**
- `team_report.html` - Responsive, interactive HTML report

**Features:**
- Professional responsive design with CSS
- Interactive collapsible sections
- Progress bars and visual indicators
- Health score with color coding
- Mobile-friendly layout
- Print-friendly styles
- Hover effects and smooth transitions

**Key Components:**
- Executive summary with metric cards
- Language breakdown with progress bars
- Critical issues table with badges
- Expandable refactoring recommendations
- Technical debt metrics with status indicators

### 🔧 SonarQube Integration (`--format sonar`)

Standard SonarQube JSON format for:
- **CI/CD pipeline integration**
- **Automated quality gates**
- **Issue tracking systems**
- **Technical debt monitoring**

**Generated Files:**
- `sonar_issues.json` - SonarQube-compatible issue format

**Issue Mapping:**
- **BLOCKER**: Critical complexity issues (score > 0.9)
- **CRITICAL**: High complexity issues (score > 0.8) 
- **MAJOR**: Moderate complexity issues (score > 0.6)
- **MINOR**: Refactoring opportunities
- **INFO**: Low-priority suggestions

**Integration Example:**
```json
{
  "issues": [
    {
      "engineId": "valknut",
      "ruleId": "complexity_analysis", 
      "type": "CODE_SMELL",
      "severity": "BLOCKER",
      "primaryLocation": {
        "message": "High complexity detected in process_data (score: 0.912)",
        "filePath": "src/analyzer.py",
        "textRange": {"startLine": 1, "endLine": 1}
      },
      "effortMinutes": 480
    }
  ]
}
```

### 📊 CSV Export (`--format csv`)

Structured data format perfect for:
- **Spreadsheet analysis**
- **Team dashboards**
- **Metrics tracking**
- **Custom reporting tools**

**Generated Files:**
- `analysis_data.csv` - Structured data with all metrics

**Columns:**
- Entity ID, Name, File Path, Language
- Complexity Score, Severity, Priority Score
- Primary Issues, Refactoring Count
- Effort Estimate (hours), Recommendations

**Use Cases:**
- Import into Excel/Google Sheets for custom analysis
- Feed into team dashboards (Grafana, Tableau)
- Track technical debt trends over time
- Generate custom visualizations

## Integration Examples

### GitHub Actions CI/CD

```yaml
name: Code Quality Analysis
on: [push, pull_request]

jobs:
  quality_check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup Python
        uses: actions/setup-python@v4
        with:
          python-version: '3.11'
      
      - name: Install Valknut
        run: pip install valknut
      
      - name: Generate Quality Report
        run: |
          valknut analyze --format html --out reports/ src/
          valknut analyze --format sonar --out quality/ src/
      
      - name: Upload HTML Report
        uses: actions/upload-artifact@v3
        with:
          name: quality-report
          path: reports/team_report.html
          
      - name: Comment PR with Results
        if: github.event_name == 'pull_request'
        run: |
          # Extract key metrics and comment on PR
          echo "Quality analysis complete. Download full report from artifacts."
```

### SonarQube Integration

```bash
# Generate SonarQube-compatible issues
valknut analyze --format sonar --out build/quality/ src/

# Import into SonarQube using generic issue format
sonar-scanner \
  -Dsonar.projectKey=myproject \
  -Dsonar.sources=src/ \
  -Dsonar.externalIssuesReportPaths=build/quality/sonar_issues.json
```

### Team Dashboard Integration

```python
import pandas as pd
import plotly.express as px

# Load CSV data for dashboard
df = pd.read_csv('reports/analysis_data.csv')

# Create complexity distribution chart
fig = px.histogram(df, x='Complexity Score', color='Language',
                  title='Complexity Distribution by Language')

# Track high-priority issues
priority_issues = df[df['Severity'].isin(['BLOCKER', 'CRITICAL'])]
print(f"High priority issues: {len(priority_issues)}")
```

## Report Structure

All team reports follow a consistent structure:

### 1. Executive Summary
- Project name and analysis date
- Overall health score (0-100)
- Key metrics overview
- Priority issues count

### 2. Language Breakdown  
- Files and entities per language
- Average and maximum complexity scores
- Refactoring suggestions count
- Health status indicators

### 3. Critical Issues
- Entities requiring immediate attention
- Severity classification
- Primary issues identification
- Effort estimates

### 4. Refactoring Recommendations
- Prioritized by impact and effort
- Grouped by refactoring type
- Real examples from codebase
- Benefits and effort estimates

### 5. Technical Debt Metrics
- Debt ratio and complexity trends  
- Comparison against targets
- Progress tracking capabilities
- Actionable insights

## Best Practices

### For Development Teams

1. **Regular Health Monitoring**
   ```bash
   # Weekly health check
   valknut analyze --format html --out weekly-reports/ src/
   ```

2. **Pre-commit Quality Gates**
   ```bash
   # Quick quality check
   valknut analyze --format csv --out pre-commit/ changed_files/
   ```

3. **Sprint Planning Integration**
   ```bash
   # Generate planning data
   valknut analyze --format markdown --out sprint-planning/ high_priority_modules/
   ```

### For Management and Stakeholders

1. **Executive Reporting**
   ```bash
   # Professional presentation format
   valknut analyze --format html --out executive/ entire_codebase/
   ```

2. **Trend Tracking**
   ```bash
   # Monthly trend analysis
   valknut analyze --format csv --out metrics/$(date +%Y-%m)/ src/
   ```

### For CI/CD Pipelines

1. **Automated Quality Gates**
   ```bash
   # Integration with quality systems
   valknut analyze --format sonar --out build/quality/ src/
   ```

2. **Regression Detection**
   ```bash
   # Compare against baseline
   valknut analyze --format json --out current/ src/
   # ... compare with previous results
   ```

## Configuration

### Custom Report Settings

Create a configuration file to customize analysis:

```yaml
# quality.yaml
ranking:
  top_k: 50
weights:
  cyclomatic_complexity: 2.0
  line_count: 1.5
  parameter_count: 1.0
briefs:
  max_entities: 20
```

Use with any format:
```bash
valknut analyze --config quality.yaml --format html --out reports/ src/
```

## Output Directory Structure

```
reports/
├── team_report.html          # HTML report (--format html)
├── team_report.md            # Markdown report (--format markdown)  
├── sonar_issues.json         # SonarQube format (--format sonar)
├── analysis_data.csv         # CSV data (--format csv)
├── summary.md                # Legacy summary (backward compatibility)
└── refactoring_guide.md      # Legacy guide (backward compatibility)
```

## Migration from Legacy Formats

The new team formats are designed to complement existing outputs:

- **Legacy JSONL/JSON formats** remain unchanged for backward compatibility
- **New team formats** provide structured, presentation-ready output
- **Both systems** can be used simultaneously
- **Existing tools** continue to work with legacy formats

## Troubleshooting

### Common Issues

1. **Large codebase performance**
   ```bash
   # Limit analysis scope
   valknut analyze --format html --out reports/ src/critical_modules/
   ```

2. **Memory usage with HTML reports**
   ```bash
   # Use CSV for large datasets
   valknut analyze --format csv --out reports/ large_codebase/
   ```

3. **CI/CD integration timeouts**
   ```bash
   # Focus on changed files only
   git diff --name-only HEAD~1 | xargs valknut analyze --format sonar --out quality/
   ```

### Performance Tips

- Use **CSV format** for large codebases (fastest generation)
- Use **HTML format** for smaller codebases (best visualization)  
- **Limit scope** to specific directories for faster analysis
- **Configure top_k** to focus on most critical issues

## Support and Feedback

For issues or feature requests related to team reporting:
- GitHub Issues: [valknut/issues](https://github.com/yourusername/valknut/issues)
- Documentation: [valknut.readthedocs.io](https://valknut.readthedocs.io)
- Examples: [valknut/examples](https://github.com/yourusername/valknut/tree/main/examples)

The team reporting system is designed to make code quality visible, actionable, and integrated into your development workflow. Whether you're presenting to stakeholders, planning sprints, or integrating with existing tools, valknut provides the professional-quality reports your team needs.