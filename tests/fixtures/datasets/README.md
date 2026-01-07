# Valknut Code Quality Testing Datasets

This directory contains datasets for testing and benchmarking valknut's Bayesian normalization system for code quality assessment.

## Directory Layout

```
datasets/
├── python/
│   ├── benchmarks/           # Synthetic workloads for complexity and normalization tests
│   │   ├── complexity_benchmark.py
│   │   └── irrefutable_complexity_test.py
│   ├── code_smells/
│   │   └── case-studies/     # Before/after smell examples from real projects
│   ├── data/                 # Tabular smell datasets (CSV)
│   │   ├── Python_LargeClassSmell_Dataset.csv
│   │   └── Python_LongMethodSmell_Dataset.csv
│   └── samples/              # Hand-written fixtures for quick experiments
│       ├── sample_bad_code.py
│       └── test_scenarios.py
├── quick_start_guide.md
└── README.md
```

## Available Datasets

### 1. Zenodo Python Code Smell Datasets (Sandouka & Aljamaan, 2023)

**Source**: https://doi.org/10.5281/zenodo.7512516
**Publication**: Sandouka R, Aljamaan H. 2023. Python code smells detection using conventional machine learning models. PeerJ Computer Science 9:e1370

#### Files (see `python/data/`):
- `Python_LargeClassSmell_Dataset.csv` (99.3 KB) - 1,000 samples of Large Class smell
- `Python_LongMethodSmell_Dataset.csv` (85.9 KB) - 1,000 samples of Long Method smell

#### Features (18 total):
- **Basic metrics**: loc, lloc, scloc, comments, single_comments, multi_comments, blanks
- **Halstead metrics**: h1, h2, n1, n2, vocabulary, length, calculated_length, volume, difficulty, effort, time, bugs
- **Label**: Binary classification (1 = smelly, 0 = clean)

#### Notes:
- Contains extracted code metrics, not actual source code
- Useful for validating valknut's metric calculations
- Can be used to test correlation between valknut's Bayesian scores and traditional ML classifications

### 2. Python Code Smells Examples (ZikaZaki)

**Source**: https://github.com/ZikaZaki/code-smells-python
**License**: Open source

#### Structure (`python/code_smells/case-studies/`):
```
case-studies/
├── employee-management-system/
├── point-of-sale/
├── vehicle-registry-system/
└── command-line-shell/
```

#### Code Smell Types Covered:
1. Magic Numbers
2. Long Method  
3. Duplicate Code
4. Large Class
5. Feature Envy
6. Inappropriate Intimacy
7. Data Clumps
8. Primitive Obsession
9. Long Parameter List

#### Files:
- `before.py` - Original code with code smells
- `after.py` - Refactored code with smells reduced
- 30 Python files total across 4 projects

#### Notes:
- Actual Python source code files
- Perfect for testing valknut's before/after improvement detection
- Covers 9 different code smell categories
- Includes practical, realistic code examples

## Test Scenarios

### Scenario 1: Metric Validation
Use the Zenodo CSV datasets to validate that valknut's code analysis produces similar metrics to the ground truth data.

### Scenario 2: Code Smell Detection
Run valknut on the `python/code_smells/case-studies/*/before.py` files to verify it flags the expected issues.

### Scenario 3: Improvement Detection  
Compare valknut scores between `before.py` and `after.py` in the case studies to validate that the Bayesian system detects improvements.

### Scenario 4: Ranking Validation
Use multiple files with known quality levels to test if valknut's Bayesian ranking correlates with expected quality rankings.

## Usage Instructions

### Setting up for testing:
1. Ensure valknut is installed and configured
2. Run analysis on individual files: `valknut analyze datasets/python/samples/sample_bad_code.py`
3. Compare results with ground truth data
4. Generate reports on correlation between valknut scores and known quality metrics

### Benchmark Tests:
1. **Metric Correlation**: Compare valknut's calculated metrics with Zenodo dataset values
2. **Binary Classification**: Test if valknut can distinguish between smelly/clean code
3. **Improvement Detection**: Verify that refactored code scores higher than original
4. **Consistency**: Ensure consistent results across multiple runs

## Dataset Limitations

### Zenodo Datasets:
- Only covers 2 types of code smells (Large Class, Long Method)  
- Contains metrics only, not source code
- Limited to 1,000 samples each
- Focused on specific Python projects

### ZikaZaki Repository:
- Small sample size (30 files across 4 projects)
- Limited to educational examples
- May not represent real-world complexity
- Only 9 code smell types covered

## Future Enhancements

1. **Larger Datasets**: Search for additional Python code quality datasets
2. **Real-world Projects**: Include analysis of popular open-source Python projects
3. **Multiple Languages**: Expand to other programming languages
4. **Automated Benchmarking**: Create scripts to run comprehensive test suites
5. **SonarQube Integration**: Compare valknut results with SonarQube analysis

## Contributing

To add new datasets:
1. Document the source and license
2. Describe the dataset structure and contents
3. Add test scenarios for the new data
4. Update this README with usage instructions
