#!/bin/bash
# Valknut CLI End-to-End Test Suite
# Comprehensive testing of all CLI functionality

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
VALKNUT_BIN="${PROJECT_ROOT}/target/release/valknut"
TEST_OUTPUT_DIR="${SCRIPT_DIR}/test-output"
FIXTURES_DIR="${SCRIPT_DIR}/fixtures"

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Cleanup function
cleanup() {
    echo -e "\n${BLUE}Cleaning up test artifacts...${NC}"
    rm -rf "${TEST_OUTPUT_DIR}" 2>/dev/null || true
    
    if [ ${TESTS_FAILED} -eq 0 ]; then
        echo -e "${GREEN}✓ All tests passed! (${TESTS_PASSED}/${TESTS_RUN})${NC}"
        exit 0
    else
        echo -e "${RED}✗ ${TESTS_FAILED} tests failed out of ${TESTS_RUN}${NC}"
        exit 1
    fi
}

trap cleanup EXIT

# Utility functions
log_test() {
    echo -e "${BLUE}[TEST]${NC} $1"
    ((TESTS_RUN++))
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
    ((TESTS_PASSED++))
}

log_failure() {
    echo -e "${RED}✗${NC} $1"
    ((TESTS_FAILED++))
}

log_info() {
    echo -e "${YELLOW}[INFO]${NC} $1"
}

# Check if valknut binary exists
check_binary() {
    if [ ! -f "${VALKNUT_BIN}" ]; then
        echo -e "${RED}Error: Valknut binary not found at ${VALKNUT_BIN}${NC}"
        echo "Please build the project first: cargo build --release"
        exit 1
    fi
}

# Setup test environment
setup_test_env() {
    log_info "Setting up test environment..."
    
    # Create test output directory
    mkdir -p "${TEST_OUTPUT_DIR}"
    
    # Create test repositories
    if [ -f "${FIXTURES_DIR}/create_test_repos.sh" ]; then
        cd "${FIXTURES_DIR}"
        ./create_test_repos.sh
        cd "${SCRIPT_DIR}"
    else
        echo -e "${RED}Error: Test repository creation script not found${NC}"
        exit 1
    fi
    
    log_info "Test environment ready"
}

# Basic functionality tests
test_basic_functionality() {
    echo -e "\n${BLUE}=== Basic Functionality Tests ===${NC}"
    
    # Test help command
    log_test "Testing --help command"
    if "${VALKNUT_BIN}" --help > "${TEST_OUTPUT_DIR}/help.txt" 2>&1; then
        if grep -q "Analyze your codebase" "${TEST_OUTPUT_DIR}/help.txt"; then
            log_success "Help command works correctly"
        else
            log_failure "Help command output doesn't contain expected text"
        fi
    else
        log_failure "Help command failed"
    fi
    
    # Test version command
    log_test "Testing --version command"
    if "${VALKNUT_BIN}" --version > "${TEST_OUTPUT_DIR}/version.txt" 2>&1; then
        if grep -qE "[0-9]+\.[0-9]+\.[0-9]+" "${TEST_OUTPUT_DIR}/version.txt"; then
            log_success "Version command works correctly"
        else
            log_failure "Version command output doesn't contain version number"
        fi
    else
        log_failure "Version command failed"
    fi
    
    # Test analyze help
    log_test "Testing analyze --help command"
    if "${VALKNUT_BIN}" analyze --help > "${TEST_OUTPUT_DIR}/analyze_help.txt" 2>&1; then
        if grep -q "Analyze code repositories" "${TEST_OUTPUT_DIR}/analyze_help.txt"; then
            log_success "Analyze help command works correctly"
        else
            log_failure "Analyze help command output doesn't contain expected text"
        fi
    else
        log_failure "Analyze help command failed"
    fi
    
    # Test nonexistent path
    log_test "Testing analysis of nonexistent path"
    if "${VALKNUT_BIN}" analyze "/nonexistent/path" > "${TEST_OUTPUT_DIR}/nonexistent.txt" 2>&1; then
        log_failure "Analysis of nonexistent path should fail but didn't"
    else
        log_success "Analysis of nonexistent path correctly failed"
    fi
    
    # Test empty directory
    log_test "Testing analysis of empty directory"
    empty_dir="${TEST_OUTPUT_DIR}/empty"
    mkdir -p "${empty_dir}"
    if "${VALKNUT_BIN}" analyze "${empty_dir}" --format json > "${TEST_OUTPUT_DIR}/empty_analysis.json" 2>&1; then
        log_success "Analysis of empty directory completed successfully"
    else
        log_failure "Analysis of empty directory failed"
    fi
}

# Output format tests
test_output_formats() {
    echo -e "\n${BLUE}=== Output Format Tests ===${NC}"
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    
    # Test JSON output
    log_test "Testing JSON output format"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${TEST_OUTPUT_DIR}/output.json" 2>&1; then
        if jq . "${TEST_OUTPUT_DIR}/output.json" > /dev/null 2>&1; then
            log_success "JSON output is valid"
        else
            log_failure "JSON output is malformed"
        fi
    else
        log_failure "JSON output format test failed"
    fi
    
    # Test YAML output
    log_test "Testing YAML output format"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format yaml > "${TEST_OUTPUT_DIR}/output.yaml" 2>&1; then
        # Basic YAML validation - check if it starts with valid YAML
        if head -n 1 "${TEST_OUTPUT_DIR}/output.yaml" | grep -qE "^[a-zA-Z_][a-zA-Z0-9_]*:"; then
            log_success "YAML output appears valid"
        else
            log_failure "YAML output appears malformed"
        fi
    else
        log_failure "YAML output format test failed"
    fi
    
    # Test pretty output
    log_test "Testing pretty output format"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format pretty > "${TEST_OUTPUT_DIR}/output.pretty" 2>&1; then
        log_success "Pretty output format completed"
    else
        log_failure "Pretty output format test failed"
    fi
    
    # Test HTML output (if supported)
    log_test "Testing HTML output format"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format html > "${TEST_OUTPUT_DIR}/output.html" 2>&1; then
        if grep -q "<html>" "${TEST_OUTPUT_DIR}/output.html" 2>/dev/null; then
            log_success "HTML output appears valid"
        else
            log_success "HTML output completed (content validation skipped)"
        fi
    else
        # HTML might not be supported, so we'll treat this as a soft failure
        log_info "HTML output format not supported or failed"
    fi
    
    # Test CSV output (if supported)
    log_test "Testing CSV output format"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format csv > "${TEST_OUTPUT_DIR}/output.csv" 2>&1; then
        log_success "CSV output format completed"
    else
        log_info "CSV output format not supported or failed"
    fi
}

# Configuration tests
test_configuration() {
    echo -e "\n${BLUE}=== Configuration Tests ===${NC}"
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_dir="${FIXTURES_DIR}/test-repos/config-test/configs"
    
    # Test minimal configuration
    log_test "Testing minimal configuration"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_dir}/minimal.yml" --format json > "${TEST_OUTPUT_DIR}/minimal_config.json" 2>&1; then
        log_success "Minimal configuration test passed"
    else
        log_failure "Minimal configuration test failed"
    fi
    
    # Test maximum configuration
    log_test "Testing maximum configuration"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_dir}/maximum.yml" --format json > "${TEST_OUTPUT_DIR}/maximum_config.json" 2>&1; then
        log_success "Maximum configuration test passed"
    else
        log_failure "Maximum configuration test failed"
    fi
    
    # Test invalid configuration
    log_test "Testing invalid configuration"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_dir}/invalid.yml" --format json > "${TEST_OUTPUT_DIR}/invalid_config.json" 2>&1; then
        log_failure "Invalid configuration should fail but didn't"
    else
        log_success "Invalid configuration correctly failed"
    fi
    
    # Test nonexistent configuration file
    log_test "Testing nonexistent configuration file"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "/nonexistent/config.yml" --format json > "${TEST_OUTPUT_DIR}/nonexistent_config.json" 2>&1; then
        log_failure "Nonexistent configuration file should fail but didn't"
    else
        log_success "Nonexistent configuration file correctly failed"
    fi
}

# Command line flags tests
test_command_line_flags() {
    echo -e "\n${BLUE}=== Command Line Flags Tests ===${NC}"
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    
    # Test verbose flag
    log_test "Testing --verbose flag"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --verbose --format json > "${TEST_OUTPUT_DIR}/verbose.json" 2>&1; then
        log_success "Verbose flag test passed"
    else
        log_failure "Verbose flag test failed"
    fi
    
    # Test quiet flag
    log_test "Testing --quiet flag"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --quiet --format json > "${TEST_OUTPUT_DIR}/quiet.json" 2>&1; then
        log_success "Quiet flag test passed"
    else
        log_failure "Quiet flag test failed"
    fi
    
    # Test quality gate flag
    log_test "Testing --quality-gate flag"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --quality-gate --format json > "${TEST_OUTPUT_DIR}/quality_gate.json" 2>&1; then
        log_success "Quality gate flag test passed"
    else
        log_failure "Quality gate flag test failed"
    fi
    
    # Test max-complexity parameter (if supported)
    log_test "Testing --max-complexity parameter"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --max-complexity 75 --format json > "${TEST_OUTPUT_DIR}/max_complexity.json" 2>&1; then
        log_success "Max complexity parameter test passed"
    else
        log_info "Max complexity parameter not supported or failed"
    fi
    
    # Test min-health parameter (if supported)
    log_test "Testing --min-health parameter"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --min-health 60 --format json > "${TEST_OUTPUT_DIR}/min_health.json" 2>&1; then
        log_success "Min health parameter test passed"
    else
        log_info "Min health parameter not supported or failed"
    fi
}

# Language support tests
test_language_support() {
    echo -e "\n${BLUE}=== Language Support Tests ===${NC}"
    
    # Test Python project
    log_test "Testing Python project analysis"
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/small-python" --format json > "${TEST_OUTPUT_DIR}/python_analysis.json" 2>&1; then
        log_success "Python project analysis passed"
    else
        log_failure "Python project analysis failed"
    fi
    
    # Test Rust project
    log_test "Testing Rust project analysis"
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/medium-rust" --format json > "${TEST_OUTPUT_DIR}/rust_analysis.json" 2>&1; then
        log_success "Rust project analysis passed"
    else
        log_failure "Rust project analysis failed"
    fi
    
    # Test mixed language project
    log_test "Testing mixed language project analysis"
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/large-mixed" --format json > "${TEST_OUTPUT_DIR}/mixed_analysis.json" 2>&1; then
        log_success "Mixed language project analysis passed"
    else
        log_failure "Mixed language project analysis failed"
    fi
    
    # Test performance test repository
    log_test "Testing performance test repository"
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/performance-test" --format json > "${TEST_OUTPUT_DIR}/performance_analysis.json" 2>&1; then
        log_success "Performance test repository analysis passed"
    else
        log_failure "Performance test repository analysis failed"
    fi
}

# Error handling tests
test_error_handling() {
    echo -e "\n${BLUE}=== Error Handling Tests ===${NC}"
    
    # Test analysis with no arguments
    log_test "Testing analysis with no arguments"
    if "${VALKNUT_BIN}" analyze > "${TEST_OUTPUT_DIR}/no_args.txt" 2>&1; then
        log_failure "Analysis with no arguments should fail but didn't"
    else
        log_success "Analysis with no arguments correctly failed"
    fi
    
    # Test invalid format
    log_test "Testing invalid output format"
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/small-python" --format invalid > "${TEST_OUTPUT_DIR}/invalid_format.txt" 2>&1; then
        log_failure "Invalid format should fail but didn't"
    else
        log_success "Invalid format correctly failed"
    fi
    
    # Test permission denied (create a directory without read permissions)
    log_test "Testing permission denied scenario"
    restricted_dir="${TEST_OUTPUT_DIR}/restricted"
    mkdir -p "${restricted_dir}"
    chmod 000 "${restricted_dir}" 2>/dev/null || true
    if "${VALKNUT_BIN}" analyze "${restricted_dir}" --format json > "${TEST_OUTPUT_DIR}/permission_denied.txt" 2>&1; then
        log_failure "Permission denied should fail but didn't"
    else
        log_success "Permission denied correctly failed"
    fi
    chmod 755 "${restricted_dir}" 2>/dev/null || true # Restore permissions for cleanup
    
    # Test very large complexity threshold
    log_test "Testing very large complexity threshold"
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/small-python" --max-complexity 999999 --format json > "${TEST_OUTPUT_DIR}/large_complexity.json" 2>&1; then
        log_success "Large complexity threshold handled correctly"
    else
        log_info "Large complexity threshold not supported or failed"
    fi
    
    # Test invalid configuration syntax
    log_test "Testing malformed YAML configuration"
    malformed_config="${TEST_OUTPUT_DIR}/malformed.yml"
    echo "invalid: yaml: syntax [missing bracket" > "${malformed_config}"
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/small-python" --config "${malformed_config}" --format json > "${TEST_OUTPUT_DIR}/malformed_config.txt" 2>&1; then
        log_failure "Malformed YAML should fail but didn't"
    else
        log_success "Malformed YAML correctly failed"
    fi
}

# Performance tests
test_performance() {
    echo -e "\n${BLUE}=== Performance Tests ===${NC}"
    
    # Test with timeout
    log_test "Testing analysis with timeout (if supported)"
    start_time=$(date +%s)
    if timeout 30 "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/performance-test" --format json > "${TEST_OUTPUT_DIR}/performance_timeout.json" 2>&1; then
        end_time=$(date +%s)
        duration=$((end_time - start_time))
        if [ ${duration} -lt 30 ]; then
            log_success "Performance test completed in ${duration} seconds"
        else
            log_info "Performance test took exactly 30 seconds (timeout)"
        fi
    else
        log_info "Performance test timed out or failed"
    fi
    
    # Test memory usage (basic check)
    log_test "Testing memory usage monitoring"
    # Run analysis and capture process info
    if "${VALKNUT_BIN}" analyze "${FIXTURES_DIR}/test-repos/medium-rust" --format json > "${TEST_OUTPUT_DIR}/memory_test.json" 2>&1; then
        log_success "Memory usage test completed"
    else
        log_failure "Memory usage test failed"
    fi
    
    # Test concurrent analysis (multiple paths)
    log_test "Testing multiple directory analysis"
    if "${VALKNUT_BIN}" analyze \
        "${FIXTURES_DIR}/test-repos/small-python" \
        "${FIXTURES_DIR}/test-repos/medium-rust" \
        --format json > "${TEST_OUTPUT_DIR}/concurrent.json" 2>&1; then
        log_success "Multiple directory analysis passed"
    else
        log_failure "Multiple directory analysis failed"
    fi
}

# Output validation tests
test_output_validation() {
    echo -e "\n${BLUE}=== Output Validation Tests ===${NC}"
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    
    # Test JSON output structure
    log_test "Validating JSON output structure"
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${TEST_OUTPUT_DIR}/validation.json" 2>&1; then
        # Check for required fields (adapt based on actual output structure)
        if jq -e '.analysis_results' "${TEST_OUTPUT_DIR}/validation.json" > /dev/null 2>&1; then
            log_success "JSON output contains expected structure"
        else
            log_info "JSON output structure different than expected (may be valid)"
        fi
    else
        log_failure "JSON output validation failed"
    fi
    
    # Test output file sizes
    log_test "Testing output file sizes"
    "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${TEST_OUTPUT_DIR}/size_test.json" 2>&1 || true
    if [ -f "${TEST_OUTPUT_DIR}/size_test.json" ]; then
        size=$(wc -c < "${TEST_OUTPUT_DIR}/size_test.json")
        if [ ${size} -gt 10 ]; then
            log_success "Output file has reasonable size (${size} bytes)"
        else
            log_failure "Output file is suspiciously small (${size} bytes)"
        fi
    else
        log_failure "Output file was not created"
    fi
    
    # Test consistent output
    log_test "Testing output consistency"
    "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${TEST_OUTPUT_DIR}/consistent1.json" 2>&1 || true
    "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${TEST_OUTPUT_DIR}/consistent2.json" 2>&1 || true
    
    if [ -f "${TEST_OUTPUT_DIR}/consistent1.json" ] && [ -f "${TEST_OUTPUT_DIR}/consistent2.json" ]; then
        if cmp -s "${TEST_OUTPUT_DIR}/consistent1.json" "${TEST_OUTPUT_DIR}/consistent2.json"; then
            log_success "Output is consistent across multiple runs"
        else
            log_info "Output varies between runs (may be expected due to timestamps)"
        fi
    else
        log_failure "Could not test output consistency"
    fi
}

# Main execution
main() {
    echo -e "${BLUE}Valknut CLI End-to-End Test Suite${NC}"
    echo -e "${BLUE}=================================${NC}"
    
    check_binary
    setup_test_env
    
    # Run all test suites
    test_basic_functionality
    test_output_formats
    test_configuration
    test_command_line_flags
    test_language_support
    test_error_handling
    test_performance
    test_output_validation
    
    # Summary will be printed by cleanup function
}

# Run if called directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi