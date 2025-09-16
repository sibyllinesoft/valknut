# Valknut Test Suite

This directory contains the comprehensive test suite for the Valknut code analysis engine, organized into logical categories for better maintainability and development workflow.

## Directory Structure

```
tests/
├── rust/                    # Rust integration tests
│   ├── cli_tests.rs        # Basic CLI integration tests
│   ├── enhanced_cli_tests.rs # Advanced CLI configuration tests
│   ├── comprehensive_clone_detection_tests.rs
│   ├── directory_health_tree_tests.rs
│   ├── clone_denoising/    # Clone detection algorithm tests
│   │   ├── phase1_weighted_shingling_tests.rs
│   │   ├── phase2_structural_gate_tests.rs
│   │   ├── phase3_stop_motifs_cache_tests.rs
│   │   └── phase4_auto_calibration_payoff_tests.rs
│   ├── fixtures/           # Test data and utilities
│   │   ├── clone_denoising_test_data.rs
│   │   └── mod.rs
│   └── ...                 # Other Rust integration tests
├── cli-e2e-tests/          # End-to-end CLI testing suite
│   ├── run_e2e_tests.sh   # Main test runner
│   ├── basic_functionality/
│   │   └── test_help_and_version.sh
│   ├── output_formats/
│   │   └── test_json_yaml_formats.sh
│   ├── configuration/
│   │   └── test_config_files.sh
│   ├── error_handling/
│   │   └── test_error_scenarios.sh
│   ├── performance/
│   │   └── test_performance_scenarios.sh
│   └── fixtures/
│       ├── create_test_repos.sh
│       └── test-repos/     # Generated test repositories
└── README.md               # This file
```

## Test Categories

### 1. Rust Integration Tests (`tests/rust/`)

Traditional Rust integration tests that verify core functionality, algorithms, and library APIs.

**Key Test Files:**
- `cli_tests.rs` - Basic CLI functionality tests
- `enhanced_cli_tests.rs` - Advanced configuration and CLI option tests
- `comprehensive_clone_detection_tests.rs` - Clone detection algorithm validation
- `directory_health_tree_tests.rs` - Directory structure analysis tests

**Clone Detection Tests:**
- `clone_denoising/phase1_*` - Weighted shingling algorithm tests
- `clone_denoising/phase2_*` - Structural gate filtering tests
- `clone_denoising/phase3_*` - Stop motifs and caching tests
- `clone_denoising/phase4_*` - Auto-calibration and payoff tests

### 2. CLI End-to-End Tests (`tests/cli-e2e-tests/`)

Comprehensive black-box testing of the CLI interface, covering all user-facing functionality.

#### Test Categories:

**Basic Functionality (`basic_functionality/`)**
- Help and version commands
- Command line argument validation
- Basic error scenarios

**Output Formats (`output_formats/`)**
- JSON output validation
- YAML output format testing
- Pretty print format testing
- HTML/CSV format testing (if supported)
- Output consistency verification

**Configuration (`configuration/`)**
- Configuration file parsing
- Minimal vs maximum configuration testing
- Invalid configuration handling
- Configuration override scenarios

**Error Handling (`error_handling/`)**
- Invalid arguments and flags
- Permission denied scenarios
- Nonexistent path handling
- Malformed input handling

**Performance (`performance/`)**
- Small repository analysis timing
- Large repository stress testing
- Memory usage monitoring
- Multiple repository handling

#### Test Fixtures

The `fixtures/` directory contains:
- `create_test_repos.sh` - Script to generate test repositories
- `test-repos/` - Generated test repositories of various sizes and languages:
  - `small-python/` - Simple Python calculator project
  - `medium-rust/` - Medium-sized Rust user management system
  - `large-mixed/` - Large project with Python backend and JavaScript frontend
  - `performance-test/` - Complex algorithms for performance testing
  - `config-test/` - Various configuration files for testing

## Running Tests

### Run All Tests

```bash
# Run all Rust integration tests
cargo test

# Run all CLI E2E tests
./tests/cli-e2e-tests/run_e2e_tests.sh

# Run specific test categories
cargo test --test cli_tests
cargo test --test comprehensive_clone_detection_tests
```

### Run Specific E2E Test Categories

```bash
# Run basic functionality tests
./tests/cli-e2e-tests/basic_functionality/test_help_and_version.sh

# Run output format tests
./tests/cli-e2e-tests/output_formats/test_json_yaml_formats.sh

# Run configuration tests
./tests/cli-e2e-tests/configuration/test_config_files.sh

# Run error handling tests
./tests/cli-e2e-tests/error_handling/test_error_scenarios.sh

# Run performance tests
./tests/cli-e2e-tests/performance/test_performance_scenarios.sh
```

### Prerequisites

**For Rust Tests:**
- Rust toolchain (stable, beta, nightly)
- Cargo with all features enabled

**For CLI E2E Tests:**
- Built valknut binary: `cargo build --release`
- Bash shell with standard utilities
- Optional: `jq` for JSON validation
- Optional: `bc` for precise time measurements

### Test Data Setup

The E2E tests automatically create test repositories when first run. To manually regenerate test data:

```bash
cd tests/cli-e2e-tests/fixtures
./create_test_repos.sh
```

## Test Development Guidelines

### Adding New Rust Integration Tests

1. Create test files in `tests/rust/`
2. Follow existing naming conventions
3. Use the shared fixtures in `tests/rust/fixtures/`
4. Update `tests/rust/mod.rs` if needed

### Adding New CLI E2E Tests

1. Choose appropriate category directory
2. Create executable bash script with descriptive name
3. Follow existing test patterns and error handling
4. Use test fixtures from `tests/cli-e2e-tests/fixtures/`
5. Update main test runner if needed

### Test Naming Conventions

**Rust Tests:**
- Use descriptive names ending in `_tests.rs`
- Group related tests in subdirectories
- Prefix test functions with `test_`

**CLI E2E Tests:**
- Use descriptive script names starting with `test_`
- Make scripts executable (`chmod +x`)
- Include clear success/failure indicators

### Error Handling Standards

**Rust Tests:**
- Use `assert!`, `assert_eq!`, and `assert_ne!` macros
- Provide descriptive failure messages
- Use `Result<(), Error>` for complex test setup

**CLI E2E Tests:**
- Return 0 for success, non-zero for failure
- Use colored output for visual feedback
- Provide clear error messages with context
- Clean up temporary files in all code paths

## Continuous Integration

### GitHub Actions Integration

The test suite is integrated with the CI pipeline in `.github/workflows/ci.yml`:

- **Rust Integration Tests**: Run on multiple platforms and Rust versions
- **CLI E2E Tests**: Run on Ubuntu with full feature coverage
- **Performance Tests**: Monitor for regressions
- **Coverage Reporting**: Track test coverage metrics

### Quality Gates

The following quality gates are enforced:

- **Test Coverage**: ≥80% line coverage for core functionality
- **Test Performance**: E2E tests must complete within reasonable time limits
- **Test Stability**: Flaky tests are identified and fixed immediately
- **Cross-Platform**: Tests pass on Linux, Windows, and macOS

## Troubleshooting

### Common Issues

**Rust Integration Tests:**
- **Build failures**: Ensure all dependencies are available
- **Test data missing**: Check `tests/rust/fixtures/` directory
- **Permission errors**: Verify file permissions in test directories

**CLI E2E Tests:**
- **Binary not found**: Run `cargo build --release` first
- **Test repositories missing**: Run `tests/cli-e2e-tests/fixtures/create_test_repos.sh`
- **Permission denied**: Ensure all scripts are executable
- **Timeout issues**: Adjust timeout values for slower systems

### Debug Mode

**Enable verbose output:**
```bash
# Rust tests
RUST_LOG=debug cargo test -- --nocapture

# CLI E2E tests  
set -x  # Add to test scripts for detailed execution trace
```

**Check test artifacts:**
```bash
# E2E test output files are in /tmp/valknut_*
ls -la /tmp/valknut_*

# View recent test logs
tail -f /tmp/valknut_*.txt
```

## Contributing

When contributing new tests:

1. **Follow existing patterns** - Maintain consistency with current test structure
2. **Document test purpose** - Include clear comments explaining what each test validates
3. **Handle edge cases** - Consider error scenarios and boundary conditions
4. **Verify cross-platform** - Ensure tests work on different operating systems
5. **Update documentation** - Keep this README and inline comments current

### Test Review Checklist

- [ ] Tests have clear, descriptive names
- [ ] Error messages provide actionable feedback
- [ ] Temporary files are cleaned up properly
- [ ] Tests are deterministic and not flaky
- [ ] Performance tests have reasonable timeout limits
- [ ] Documentation is updated for new test categories

## Performance Considerations

The test suite is designed for both development speed and comprehensive coverage:

- **Fast feedback**: Basic tests complete in under 30 seconds
- **Parallel execution**: Independent tests can run concurrently
- **Incremental testing**: Target specific test categories during development
- **Resource efficiency**: Tests clean up after themselves
- **Scalable**: Test structure supports growing codebase complexity

For optimal development workflow, run fast tests frequently and comprehensive tests before commits.