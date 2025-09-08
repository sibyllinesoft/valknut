# Quick Start Guide: Testing Valknut with Code Quality Datasets

## Overview
This guide helps you quickly get started with testing valknut's Bayesian normalization system using the prepared datasets.

## Prerequisites
- Valknut installed and configured
- Python 3.7+ with pandas library
- Access to the datasets in this directory

## Quick Test Commands

### 1. Test on Sample Bad Code
```bash
# Run valknut on our intentionally bad code sample
valknut analyze datasets/sample_bad_code.py

# Expected: Should detect multiple code smells and provide low quality scores
```

### 2. Test Before/After Improvement Detection
```bash
# Analyze a "before" file (should show poor quality)
valknut analyze datasets/code-smells-python/employee-management-system/before.py

# Analyze the corresponding "after" file (should show improved quality)
valknut analyze datasets/code-smells-python/employee-management-system/after.py

# Compare the scores - "after" should have better ratings
```

### 3. Run Comprehensive Benchmark
```bash
# Execute the full benchmark suite
cd datasets
python test_scenarios.py

# This will generate benchmark_report.json with detailed results
```

## Expected Results

### Good Quality Indicators
Valknut should give **higher scores** to:
- `after.py` files (refactored code)
- Code with clear variable names
- Short, focused functions
- Proper error handling
- Good separation of concerns

### Poor Quality Indicators  
Valknut should give **lower scores** to:
- `before.py` files (code with smells)
- `sample_bad_code.py` (intentionally poor)
- Code with magic numbers
- Long methods and large classes
- Duplicate code patterns

## Quick Validation Checklist

- [ ] Valknut detects magic numbers in `sample_bad_code.py`
- [ ] `after.py` files score higher than corresponding `before.py` files
- [ ] Large class examples get lower scores than focused classes
- [ ] Long methods get lower scores than short, focused methods
- [ ] Duplicate code is penalized appropriately

## Dataset Overview

| Dataset | Type | Size | Use Case |
|---------|------|------|----------|
| Zenodo CSVs | Metrics only | 2,000 samples | Validate metric calculations |
| ZikaZaki Repo | Source code | 30 files | Before/after comparisons |
| Sample Bad Code | Source code | 1 file | Quick smoke test |

## Troubleshooting

### If valknut command not found:
```bash
# Make sure valknut is installed and in PATH
which valknut
pip install valknut  # or your installation method
```

### If benchmark script fails:
```bash
# Install required dependencies
pip install pandas

# Check if datasets are present
ls -la datasets/
```

### If results seem inconsistent:
- Run multiple times to check for consistency
- Verify valknut configuration
- Check if any files were modified during testing

## Next Steps

1. **Analyze Results**: Review `benchmark_report.json` for detailed findings
2. **Tune Parameters**: Adjust valknut configuration based on results
3. **Add More Data**: Include your own code samples for testing
4. **Compare Tools**: Run other static analysis tools for comparison
5. **Document Findings**: Create reports on valknut's performance

## Interpreting Valknut Scores

- **High Scores (0.8-1.0)**: Excellent code quality
- **Medium Scores (0.5-0.7)**: Acceptable with room for improvement  
- **Low Scores (0.0-0.4)**: Poor quality, needs refactoring

## Getting Help

- Check valknut documentation for configuration options
- Review the full dataset documentation in `README.md`
- Examine `test_scenarios.py` for detailed benchmark logic
- Compare your results with expected patterns outlined above