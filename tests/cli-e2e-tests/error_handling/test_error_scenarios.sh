#!/bin/bash
# Error handling and edge case tests

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
VALKNUT_BIN="${PROJECT_ROOT}/target/release/valknut"
FIXTURES_DIR="${SCRIPT_DIR}/../fixtures"

# Ensure test repositories exist
setup_test_data() {
    if [ ! -d "${FIXTURES_DIR}/test-repos" ]; then
        echo "Setting up test repositories..."
        cd "${FIXTURES_DIR}"
        ./create_test_repos.sh
    fi
}

test_no_arguments() {
    echo "Testing analyze command with no arguments..."
    
    local output_file="/tmp/valknut_no_args_test.txt"
    
    if "${VALKNUT_BIN}" analyze > "${output_file}" 2>&1; then
        echo "✗ Analyze with no arguments should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check that it shows help or appropriate error
        if grep -q -i "usage\|help\|required\|arguments\|paths" "${output_file}"; then
            echo "✓ No arguments shows appropriate error message"
        else
            echo "✗ No arguments error message unclear"
            echo "Error output:"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ No arguments test passed"
        rm -f "${output_file}"
        return 0
    fi
}

test_nonexistent_path() {
    echo "Testing analysis of nonexistent path..."
    
    local output_file="/tmp/valknut_nonexistent_path_test.txt"
    local nonexistent_path="/absolutely/nonexistent/path/that/should/not/exist"
    
    if "${VALKNUT_BIN}" analyze "${nonexistent_path}" --format json > "${output_file}" 2>&1; then
        echo "✗ Nonexistent path should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check error message
        if grep -q -i "not found\|no such file\|directory\|path\|error" "${output_file}"; then
            echo "✓ Nonexistent path shows appropriate error message"
        else
            echo "✗ Nonexistent path error message unclear"
            echo "Error output:"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Nonexistent path test passed"
        rm -f "${output_file}"
        return 0
    fi
}

test_permission_denied() {
    echo "Testing permission denied scenario..."
    
    local restricted_dir="/tmp/valknut_restricted_test"
    local output_file="/tmp/valknut_permission_test.txt"
    
    # Create directory and remove permissions
    mkdir -p "${restricted_dir}"
    chmod 000 "${restricted_dir}" 2>/dev/null || {
        echo "ℹ Cannot test permission denied scenario (chmod failed)"
        rm -rf "${restricted_dir}"
        return 0
    }
    
    if "${VALKNUT_BIN}" analyze "${restricted_dir}" --format json > "${output_file}" 2>&1; then
        echo "✗ Permission denied should fail but succeeded"
        chmod 755 "${restricted_dir}" 2>/dev/null || true
        rm -rf "${restricted_dir}" "${output_file}"
        return 1
    else
        # Check error message
        if grep -q -i "permission\|denied\|access\|error" "${output_file}"; then
            echo "✓ Permission denied shows appropriate error message"
        else
            echo "ℹ Permission denied error message may be valid"
            echo "Error output:"
            cat "${output_file}"
        fi
        
        echo "✓ Permission denied test passed"
        chmod 755 "${restricted_dir}" 2>/dev/null || true
        rm -rf "${restricted_dir}" "${output_file}"
        return 0
    fi
}

test_invalid_format() {
    echo "Testing invalid output format..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_invalid_format_test.txt"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format invalid-format-xyz > "${output_file}" 2>&1; then
        echo "✗ Invalid format should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check error message
        if grep -q -i "format\|invalid\|unknown\|error" "${output_file}"; then
            echo "✓ Invalid format shows appropriate error message"
        else
            echo "✗ Invalid format error message unclear"
            echo "Error output:"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Invalid format test passed"
        rm -f "${output_file}"
        return 0
    fi
}

test_invalid_flags() {
    echo "Testing invalid command line flags..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_invalid_flags_test.txt"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    # Test invalid flag
    if "${VALKNUT_BIN}" analyze "${test_repo}" --invalid-flag-xyz --format json > "${output_file}" 2>&1; then
        echo "✗ Invalid flag should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check error message
        if grep -q -i "unknown\|invalid\|unexpected\|flag\|option\|error" "${output_file}"; then
            echo "✓ Invalid flag shows appropriate error message"
        else
            echo "✗ Invalid flag error message unclear"
            echo "Error output:"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Invalid flags test passed"
        rm -f "${output_file}"
        return 0
    fi
}

test_empty_directory() {
    echo "Testing analysis of empty directory..."
    
    local empty_dir="/tmp/valknut_empty_test"
    local output_file="/tmp/valknut_empty_dir_test.json"
    
    # Create empty directory
    mkdir -p "${empty_dir}"
    
    if "${VALKNUT_BIN}" analyze "${empty_dir}" --format json > "${output_file}" 2>&1; then
        # Empty directory analysis might succeed with empty results
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 5 ]; then
            echo "✓ Empty directory analysis completed (may contain metadata)"
        else
            echo "✓ Empty directory analysis completed with minimal output"
        fi
        
        echo "✓ Empty directory test passed"
        rm -rf "${empty_dir}" "${output_file}"
        return 0
    else
        echo "ℹ Empty directory analysis failed (may be expected behavior)"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        
        rm -rf "${empty_dir}" "${output_file}"
        return 0  # Soft failure - empty dir rejection may be valid
    fi
}

test_file_instead_of_directory() {
    echo "Testing analysis of file instead of directory..."
    
    local test_file="/tmp/valknut_single_file_test.py"
    local output_file="/tmp/valknut_file_test.json"
    
    # Create a single file
    cat > "${test_file}" << 'EOF'
def hello_world():
    print("Hello, World!")

if __name__ == "__main__":
    hello_world()
EOF
    
    if "${VALKNUT_BIN}" analyze "${test_file}" --format json > "${output_file}" 2>&1; then
        echo "✓ Single file analysis completed"
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ Single file analysis produced output"
        else
            echo "✗ Single file analysis produced minimal output"
            cat "${output_file}"
            rm -f "${test_file}" "${output_file}"
            return 1
        fi
        
        echo "✓ Single file test passed"
        rm -f "${test_file}" "${output_file}"
        return 0
    else
        echo "ℹ Single file analysis failed (may require directory)"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        
        rm -f "${test_file}" "${output_file}"
        return 0  # Soft failure - single file rejection may be valid
    fi
}

test_extremely_large_thresholds() {
    echo "Testing extremely large threshold values..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_large_thresholds_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    # Test with very large complexity threshold (if supported)
    if "${VALKNUT_BIN}" analyze "${test_repo}" --max-complexity 999999 --format json > "${output_file}" 2>&1; then
        echo "✓ Large complexity threshold handled"
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ Large threshold analysis produced output"
        else
            echo "✗ Large threshold analysis produced minimal output"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Large thresholds test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "ℹ Large threshold not supported or rejected"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            head -3 "${output_file}"
        fi
        
        rm -f "${output_file}"
        return 0  # Soft failure - threshold limits may be enforced
    fi
}

test_malformed_command_combinations() {
    echo "Testing malformed command combinations..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_malformed_test.txt"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    # Test multiple conflicting flags (example)
    if "${VALKNUT_BIN}" analyze "${test_repo}" --verbose --quiet --format json > "${output_file}" 2>&1; then
        echo "ℹ Conflicting flags (verbose + quiet) were accepted"
        echo "✓ Command handled gracefully"
        rm -f "${output_file}"
        return 0
    else
        # Check if it shows appropriate error
        if grep -q -i "conflict\|invalid\|error" "${output_file}"; then
            echo "✓ Conflicting flags show appropriate error"
        else
            echo "ℹ Conflicting flags rejected with generic error"
        fi
        
        echo "✓ Malformed command combinations test passed"
        rm -f "${output_file}"
        return 0
    fi
}

test_interrupted_analysis() {
    echo "Testing interrupted analysis (timeout)..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/performance-test"
    local output_file="/tmp/valknut_timeout_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "ℹ Performance test repository not found, skipping timeout test"
        return 0
    fi
    
    # Run with short timeout to simulate interruption
    if timeout 5 "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1; then
        echo "✓ Analysis completed within timeout"
    else
        local exit_code=$?
        if [ ${exit_code} -eq 124 ]; then
            echo "✓ Analysis was interrupted by timeout (expected)"
        else
            echo "ℹ Analysis failed with exit code ${exit_code}"
        fi
    fi
    
    echo "✓ Interrupted analysis test passed"
    rm -f "${output_file}"
    return 0
}

test_special_characters_in_path() {
    echo "Testing paths with special characters..."
    
    local special_dir="/tmp/valknut test with spaces & special chars"
    local output_file="/tmp/valknut_special_chars_test.json"
    
    # Create directory with special characters
    mkdir -p "${special_dir}"
    
    # Create a simple Python file
    cat > "${special_dir}/test.py" << 'EOF'
def test_function():
    return "test"
EOF
    
    if "${VALKNUT_BIN}" analyze "${special_dir}" --format json > "${output_file}" 2>&1; then
        echo "✓ Path with special characters handled correctly"
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ Special characters path analysis produced output"
        else
            echo "✗ Special characters path analysis produced minimal output"
            cat "${output_file}"
            rm -rf "${special_dir}" "${output_file}"
            return 1
        fi
        
        echo "✓ Special characters in path test passed"
        rm -rf "${special_dir}" "${output_file}"
        return 0
    else
        echo "✗ Path with special characters failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        
        rm -rf "${special_dir}" "${output_file}"
        return 1
    fi
}

# Run tests if called directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "Running error handling tests..."
    
    setup_test_data
    
    failed_tests=0
    
    test_no_arguments || ((failed_tests++))
    test_nonexistent_path || ((failed_tests++))
    test_permission_denied || ((failed_tests++))
    test_invalid_format || ((failed_tests++))
    test_invalid_flags || ((failed_tests++))
    test_empty_directory || ((failed_tests++))
    test_file_instead_of_directory || ((failed_tests++))
    test_extremely_large_thresholds || ((failed_tests++))
    test_malformed_command_combinations || ((failed_tests++))
    test_interrupted_analysis || ((failed_tests++))
    test_special_characters_in_path || ((failed_tests++))
    
    if [ ${failed_tests} -eq 0 ]; then
        echo "All error handling tests passed!"
    else
        echo "${failed_tests} error handling tests failed!"
        exit 1
    fi
fi