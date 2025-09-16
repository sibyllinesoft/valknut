# Valknut E2E Test Suite

Comprehensive end-to-end testing for the valknut HTML generation pipeline, designed to catch issues like "No Refactoring Candidates Found" by testing the complete flow from JSON analysis results to rendered HTML.

## Overview

This E2E test suite replicates the exact same rendering pipeline as the real valknut system:

1. **Real JSON Input**: Uses actual valknut analysis results (`/tmp/analysis-results.json`)
2. **Handlebars Compilation**: Uses the real Handlebars templates from `templates/partials/tree.hbs`
3. **Complete HTML Generation**: Produces full HTML output identical to production
4. **Comprehensive Validation**: Tests tree structure, metrics display, and edge cases

## Quick Start

### Prerequisites

1. **Generate real analysis results**:
   ```bash
   cd /path/to/valknut
   cargo run -- analyze --format json --out /tmp ./src
   ```

2. **Install Node dependencies**:
   ```bash
   cd templates/assets/tests/e2e
   npm install
   ```

### Run Tests

```bash
# Run complete test suite
npm test

# Run with detailed output
npm run test:verbose

# Generate HTML outputs only (no tests)
npm run generate

# Show available options
npm run help
```

## Test Categories

### 1. Integration Tests
- **Real JSON Processing**: Validates loading and parsing of actual valknut output
- **Template Compilation**: Tests Handlebars template compilation with real data
- **Tree Structure Generation**: Verifies hierarchical tree building from flat file lists
- **Refactoring Candidates Display**: Tests both "candidates found" and "no candidates" scenarios
- **Complete HTML Generation**: Validates full page generation with embedded styles

### 2. Tree Rendering Tests  
- **Tree Node Generation**: Ensures individual tree nodes are created correctly
- **Tree Structure Hierarchy**: Validates proper nesting of directories and files
- **Tree Metrics Display**: Tests display of complexity scores and health metrics
- **Tree Interactivity**: Verifies Bootstrap collapse/expand functionality

### 3. Real-World Scenario Tests
- **No Refactoring Candidates Scenario**: Specifically tests the "No Refactoring Candidates Found" issue
- **Large Codebase Scenario**: Performance testing with real data sizes
- **Empty Directory Scenario**: Edge case handling for empty project structures

### 4. Validation Pipeline
- **HTML Structure Validation**: DOM parsing and structure verification
- **Content Validation**: Ensures expected data appears in rendered output
- **Interactive Elements Validation**: Tests Bootstrap components and JavaScript

## Generated Outputs

The test suite generates several files for manual inspection:

- **`/tmp/e2e-test-complete.html`**: Complete rendered HTML page (open in browser)
- **`/tmp/e2e-test-tree-fragment.html`**: Tree component HTML fragment  
- **`/tmp/e2e-test-template-data.json`**: Template data structure used for rendering
- **`/tmp/e2e-test-results.json`**: Complete test execution results and metrics

## Architecture

### Core Components

- **`test-runner.js`**: Main orchestrator that runs all test categories
- **`html-generator.js`**: Replicates the complete valknut HTML generation pipeline
- **`template-compiler.js`**: Handlebars compilation with real valknut helpers and partials
- **`tree-validation.js`**: DOM-based validation of rendered HTML structure
- **`integration-tests.js`**: Comprehensive integration test scenarios
- **`run-e2e.js`**: Command-line entry point with options

### Template Compilation

The test suite uses the exact same Handlebars setup as production:

```javascript
// Registers all valknut helpers
this.handlebars.registerHelper('json', function(context) { ... });
this.handlebars.registerHelper('eq', function(a, b) { ... });
this.handlebars.registerHelper('formatScore', function(score) { ... });

// Registers all partials from templates/partials/
this.handlebars.registerPartial('tree', treeTemplateContent);
this.handlebars.registerPartial('footer', footerTemplateContent);
```

### Data Transformation

Real valknut JSON is transformed to match template expectations:

```javascript
const transformed = {
    summary: analysisResults.summary,
    files: analysisResults.files,
    refactoring_candidates: analysisResults.refactoring_candidates,
    tree_data: this.buildTreeStructure(analysisResults.files)
};
```

## Debugging Failed Tests

### Common Issues

1. **"Template not found"**: Ensure you're running from the correct directory and `templates/partials/tree.hbs` exists
2. **"Real analysis results not found"**: Run `valknut analyze --format json --out /tmp ./src` first
3. **"No tree nodes found"**: Check that your analysis results contain actual files in the `files` array
4. **"Missing refactoring candidates message"**: This indicates the original bug - the template isn't handling empty candidates correctly

### Investigation Steps

1. **Check generated outputs**:
   ```bash
   # Run tests to generate outputs
   npm test
   
   # Open complete HTML in browser
   open /tmp/e2e-test-complete.html
   
   # Examine template data structure
   cat /tmp/e2e-test-template-data.json | jq .
   ```

2. **Run with verbose output**:
   ```bash
   npm run test:verbose
   ```

3. **Generate outputs only** (skip tests):
   ```bash
   npm run generate
   ```

## Integration with CI/CD

### Exit Codes
- `0`: All tests passed
- `1`: Some tests failed
- `2`: Test suite failed to run (setup issues)
- `3`: Uncaught exception or unhandled rejection
- `4`: Fatal error

### Example CI Integration

```yaml
# .github/workflows/e2e-tests.yml
- name: Generate Analysis Results
  run: |
    cargo run -- analyze --format json --out /tmp ./src

- name: Run E2E Tests  
  run: |
    cd templates/assets/tests/e2e
    npm install
    npm test

- name: Upload Test Artifacts
  if: always()
  uses: actions/upload-artifact@v3
  with:
    name: e2e-test-outputs
    path: /tmp/e2e-test-*.html
```

## Extending the Test Suite

### Adding New Tests

1. **Add to existing categories** in `integration-tests.js`:
   ```javascript
   testNewScenario() {
       const testName = 'New Scenario';
       // Test implementation
       this.recordTestResult(testName, passed, details);
   }
   ```

2. **Add new test categories** in `test-runner.js`:
   ```javascript
   async runNewTestCategory() {
       const tests = [
           () => this.testNewFeature(),
           () => this.testAnotherFeature()
       ];
       // Execute tests
   }
   ```

### Adding New Validations

Extend `tree-validation.js` with new validation rules:

```javascript
validateNewFeature(document, expectedData) {
    // Custom validation logic
    if (!isValid) {
        this.errors.push('New feature validation failed');
    }
}
```

## Troubleshooting

### Missing Dependencies

```bash
# Install required packages
npm install handlebars jsdom

# Or install globally
npm install -g handlebars jsdom
```

### Permission Issues

```bash
# Make run script executable
chmod +x run-e2e.js

# Run directly with node
node run-e2e.js
```

### Template Loading Issues

Ensure your working directory structure:
```
templates/
├── partials/
│   ├── tree.hbs
│   └── footer.hbs
└── assets/
    └── tests/
        └── e2e/
            ├── run-e2e.js
            └── ...
```

The test suite expects to find templates at `../../../partials/` relative to the E2E test directory.

## Contributing

When modifying the test suite:

1. **Maintain real-world accuracy**: Tests should replicate the exact production pipeline
2. **Add validation for new features**: Any new valknut features should have corresponding E2E tests  
3. **Test edge cases**: Empty data, large datasets, malformed input
4. **Update documentation**: Keep this README current with any new test categories or options

## Performance Considerations

- **Template compilation**: Cached after first load
- **Large datasets**: Tests validate performance doesn't degrade significantly
- **Memory usage**: Tests monitor memory consumption for large codebases
- **Generation time**: HTML generation should complete within reasonable time limits

The E2E test suite is designed to catch regressions early and ensure the valknut HTML generation pipeline remains robust across different scenarios and data sizes.