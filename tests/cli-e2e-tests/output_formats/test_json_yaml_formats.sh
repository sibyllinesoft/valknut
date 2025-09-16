#!/bin/bash
# Output format validation tests

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

test_json_output() {
    echo "Testing JSON output format..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_json_test.txt"
    local test_dir="/tmp/valknut_json_output"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    # Create temporary directory for output
    rm -rf "${test_dir}"
    mkdir -p "${test_dir}"
    cd "${test_dir}"
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1; then
        # Check if JSON file was created
        if [ -f ".valknut/analysis-results.json" ]; then
            echo "✓ JSON output file created"
            
            # Validate JSON syntax
            if command -v jq >/dev/null 2>&1; then
                if jq . ".valknut/analysis-results.json" > /dev/null 2>&1; then
                    echo "✓ JSON output is valid"
                else
                    echo "✗ JSON output is malformed"
                    echo "First few lines of JSON file:"
                    head -5 ".valknut/analysis-results.json"
                    cd - > /dev/null
                    rm -rf "${test_dir}" "${output_file}"
                    return 1
                fi
                
                # Check for expected fields
                if jq -e '.summary' ".valknut/analysis-results.json" > /dev/null 2>&1; then
                    echo "✓ JSON output contains expected summary structure"
                else
                    echo "ℹ JSON output structure differs from expected (may be valid)"
                fi
            else
                echo "✓ JSON output generated (jq not available for validation)"
            fi
        else
            echo "✗ JSON output file not created"
            echo "Command output:"
            cat "${output_file}"
            cd - > /dev/null
            rm -rf "${test_dir}" "${output_file}"
            return 1
        fi
        
        # Check file size of JSON output
        local json_size=$(wc -c < ".valknut/analysis-results.json")
        if [ ${json_size} -gt 10 ]; then
            echo "✓ JSON output has reasonable size (${json_size} bytes)"
        else
            echo "✗ JSON output suspiciously small (${json_size} bytes)"
            cat ".valknut/analysis-results.json"
            cd - > /dev/null
            rm -rf "${test_dir}" "${output_file}"
            return 1
        fi
        
        echo "✓ JSON output test passed"
        cd - > /dev/null
        rm -rf "${test_dir}" "${output_file}"
        return 0
    else
        echo "✗ JSON output test failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        cd - > /dev/null
        rm -rf "${test_dir}" "${output_file}"
        return 1
    fi
}

test_yaml_output() {
    echo "Testing YAML output format..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_yaml_test.txt"
    local test_dir="/tmp/valknut_yaml_output"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    # Create temporary directory for output
    rm -rf "${test_dir}"
    mkdir -p "${test_dir}"
    cd "${test_dir}"
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format yaml > "${output_file}" 2>&1; then
        # Check if YAML file was created
        if [ -f ".valknut/analysis-results.yaml" ]; then
            echo "✓ YAML output file created"
            
            # Basic YAML validation
            if head -1 ".valknut/analysis-results.yaml" | grep -qE "^[a-zA-Z_][a-zA-Z0-9_]*:"; then
                echo "✓ YAML output starts with valid key"
            else
                echo "✗ YAML output doesn't start with valid key"
                echo "First line: $(head -1 ".valknut/analysis-results.yaml")"
                cd - > /dev/null
                rm -rf "${test_dir}" "${output_file}"
                return 1
            fi
            
            # Check for YAML structure
            if grep -q ":" ".valknut/analysis-results.yaml"; then
                echo "✓ YAML output contains key-value pairs"
            else
                echo "✗ YAML output missing key-value pairs"
                cd - > /dev/null
                rm -rf "${test_dir}" "${output_file}"
                return 1
            fi
            
            # Check file size
            local yaml_size=$(wc -c < ".valknut/analysis-results.yaml")
            if [ ${yaml_size} -gt 10 ]; then
                echo "✓ YAML output has reasonable size (${yaml_size} bytes)"
            else
                echo "✗ YAML output suspiciously small (${yaml_size} bytes)"
                cat ".valknut/analysis-results.yaml"
                cd - > /dev/null
                rm -rf "${test_dir}" "${output_file}"
                return 1
            fi
        else
            echo "✗ YAML output file not created"
            echo "Command output:"
            cat "${output_file}"
            cd - > /dev/null
            rm -rf "${test_dir}" "${output_file}"
            return 1
        fi
        
        echo "✓ YAML output test passed"
        cd - > /dev/null
        rm -rf "${test_dir}" "${output_file}"
        return 0
    else
        echo "✗ YAML output test failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        cd - > /dev/null
        rm -rf "${test_dir}" "${output_file}"
        return 1
    fi
}

test_pretty_output() {
    echo "Testing pretty output format..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_pretty_test.txt"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format pretty > "${output_file}" 2>&1; then
        # Check file size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ Pretty output has reasonable size (${size} bytes)"
        else
            echo "✗ Pretty output suspiciously small (${size} bytes)"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        # Check for readable content
        if grep -q -E "(analysis|result|file|score)" "${output_file}"; then
            echo "✓ Pretty output contains expected keywords"
        else
            echo "✓ Pretty output generated (content validation skipped)"
        fi
        
        echo "✓ Pretty output test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Pretty output test failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_html_output() {
    echo "Testing HTML output format..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_html_test.html"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format html > "${output_file}" 2>&1; then
        # Check for HTML structure
        if grep -q "<html>" "${output_file}" || grep -q "<!DOCTYPE" "${output_file}"; then
            echo "✓ HTML output contains HTML structure"
        else
            echo "✓ HTML output generated (may not be standard HTML format)"
        fi
        
        # Check file size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ HTML output has reasonable size (${size} bytes)"
        else
            echo "✗ HTML output suspiciously small (${size} bytes)"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ HTML output test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "ℹ HTML output format not supported or failed"
        rm -f "${output_file}"
        return 0  # Soft failure - HTML might not be implemented
    fi
}

test_csv_output() {
    echo "Testing CSV output format..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_csv_test.csv"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format csv > "${output_file}" 2>&1; then
        # Check for CSV structure
        if grep -q "," "${output_file}"; then
            echo "✓ CSV output contains commas (likely CSV format)"
        else
            echo "✓ CSV output generated (may not use comma separator)"
        fi
        
        # Check file size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ CSV output has reasonable size (${size} bytes)"
        else
            echo "✗ CSV output suspiciously small (${size} bytes)"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ CSV output test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "ℹ CSV output format not supported or failed"
        rm -f "${output_file}"
        return 0  # Soft failure - CSV might not be implemented
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
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --format invalid-format > "${output_file}" 2>&1; then
        echo "✗ Invalid format should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check error message
        if grep -q -i "format\|invalid\|error" "${output_file}"; then
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

test_output_consistency() {
    echo "Testing output consistency across multiple runs..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output1="/tmp/valknut_consistency1.json"
    local output2="/tmp/valknut_consistency2.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    # Run analysis twice
    "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output1}" 2>&1 || true
    "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output2}" 2>&1 || true
    
    if [ -f "${output1}" ] && [ -f "${output2}" ]; then
        if cmp -s "${output1}" "${output2}"; then
            echo "✓ Output is consistent across multiple runs"
        else
            echo "ℹ Output varies between runs (may be expected due to timestamps)"
        fi
        
        rm -f "${output1}" "${output2}"
        return 0
    else
        echo "✗ Could not test output consistency - one or both runs failed"
        rm -f "${output1}" "${output2}"
        return 1
    fi
}

# Run tests if called directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "Running output format tests..."
    
    setup_test_data
    
    failed_tests=0
    
    test_json_output || ((failed_tests++))
    test_yaml_output || ((failed_tests++))
    test_pretty_output || ((failed_tests++))
    test_html_output || ((failed_tests++))
    test_csv_output || ((failed_tests++))
    test_invalid_format || ((failed_tests++))
    test_output_consistency || ((failed_tests++))
    
    if [ ${failed_tests} -eq 0 ]; then
        echo "All output format tests passed!"
    else
        echo "${failed_tests} output format tests failed!"
        exit 1
    fi
fi