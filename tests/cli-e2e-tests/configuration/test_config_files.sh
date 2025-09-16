#!/bin/bash
# Configuration file tests

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

test_minimal_config() {
    echo "Testing minimal configuration..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_file="${FIXTURES_DIR}/test-repos/config-test/configs/minimal.yml"
    local output_file="/tmp/valknut_minimal_config_test.json"
    
    if [ ! -f "${config_file}" ]; then
        echo "✗ Minimal config file not found: ${config_file}"
        return 1
    fi
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" --format json > "${output_file}" 2>&1; then
        # Check that output was generated
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ Minimal configuration analysis completed successfully"
        else
            echo "✗ Minimal configuration analysis produced no output"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Minimal configuration test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Minimal configuration test failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_maximum_config() {
    echo "Testing maximum configuration..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_file="${FIXTURES_DIR}/test-repos/config-test/configs/maximum.yml"
    local output_file="/tmp/valknut_maximum_config_test.json"
    
    if [ ! -f "${config_file}" ]; then
        echo "✗ Maximum config file not found: ${config_file}"
        return 1
    fi
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" --format json > "${output_file}" 2>&1; then
        # Check that output was generated
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ Maximum configuration analysis completed successfully"
        else
            echo "✗ Maximum configuration analysis produced no output"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Maximum configuration test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Maximum configuration test failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_invalid_config() {
    echo "Testing invalid configuration..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_file="${FIXTURES_DIR}/test-repos/config-test/configs/invalid.yml"
    local output_file="/tmp/valknut_invalid_config_test.txt"
    
    if [ ! -f "${config_file}" ]; then
        echo "✗ Invalid config file not found: ${config_file}"
        return 1
    fi
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" --format json > "${output_file}" 2>&1; then
        echo "✗ Invalid configuration should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check error message contains configuration-related terms
        if grep -q -i "config\|configuration\|invalid\|error" "${output_file}"; then
            echo "✓ Invalid configuration shows appropriate error message"
        else
            echo "✗ Invalid configuration error message unclear"
            echo "Error output:"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Invalid configuration test passed"
        rm -f "${output_file}"
        return 0
    fi
}

test_nonexistent_config() {
    echo "Testing nonexistent configuration file..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_file="/nonexistent/config.yml"
    local output_file="/tmp/valknut_nonexistent_config_test.txt"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" --format json > "${output_file}" 2>&1; then
        echo "✗ Nonexistent configuration file should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check error message
        if grep -q -i "config\|file\|not found\|error" "${output_file}"; then
            echo "✓ Nonexistent configuration file shows appropriate error message"
        else
            echo "✗ Nonexistent configuration file error message unclear"
            echo "Error output:"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Nonexistent configuration file test passed"
        rm -f "${output_file}"
        return 0
    fi
}

test_malformed_yaml_config() {
    echo "Testing malformed YAML configuration..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_file="/tmp/valknut_malformed_config.yml"
    local output_file="/tmp/valknut_malformed_config_test.txt"
    
    # Create malformed YAML
    cat > "${config_file}" << 'EOF'
analysis:
  enable_scoring: true
  invalid_yaml: [missing_closing_bracket
  another_key: value
EOF
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        rm -f "${config_file}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" --format json > "${output_file}" 2>&1; then
        echo "✗ Malformed YAML configuration should fail but succeeded"
        rm -f "${config_file}" "${output_file}"
        return 1
    else
        # Check error message
        if grep -q -i "yaml\|config\|parse\|syntax\|error" "${output_file}"; then
            echo "✓ Malformed YAML configuration shows appropriate error message"
        else
            echo "✗ Malformed YAML configuration error message unclear"
            echo "Error output:"
            cat "${output_file}"
            rm -f "${config_file}" "${output_file}"
            return 1
        fi
        
        echo "✓ Malformed YAML configuration test passed"
        rm -f "${config_file}" "${output_file}"
        return 0
    fi
}

test_empty_config() {
    echo "Testing empty configuration file..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_file="/tmp/valknut_empty_config.yml"
    local output_file="/tmp/valknut_empty_config_test.json"
    
    # Create empty config file
    touch "${config_file}"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        rm -f "${config_file}"
        return 1
    fi
    
    if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" --format json > "${output_file}" 2>&1; then
        echo "ℹ Empty configuration file was accepted (may use defaults)"
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "✓ Empty configuration produced output (using defaults)"
        else
            echo "✗ Empty configuration produced no output"
            cat "${output_file}"
            rm -f "${config_file}" "${output_file}"
            return 1
        fi
        
        echo "✓ Empty configuration test passed"
        rm -f "${config_file}" "${output_file}"
        return 0
    else
        echo "ℹ Empty configuration file was rejected (may require minimum config)"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            cat "${output_file}"
        fi
        
        rm -f "${config_file}" "${output_file}"
        return 0  # Soft failure - empty config rejection may be valid
    fi
}

test_config_with_different_repos() {
    echo "Testing configuration with different repository types..."
    
    local config_file="${FIXTURES_DIR}/test-repos/config-test/configs/maximum.yml"
    local repos=(
        "small-python"
        "medium-rust"
        "large-mixed"
    )
    
    if [ ! -f "${config_file}" ]; then
        echo "✗ Config file not found: ${config_file}"
        return 1
    fi
    
    local failed=0
    for repo in "${repos[@]}"; do
        local test_repo="${FIXTURES_DIR}/test-repos/${repo}"
        local output_file="/tmp/valknut_config_${repo}_test.json"
        
        if [ ! -d "${test_repo}" ]; then
            echo "✗ Test repository not found: ${test_repo}"
            ((failed++))
            continue
        fi
        
        echo "  Testing with ${repo}..."
        if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" --format json > "${output_file}" 2>&1; then
            local size=$(wc -c < "${output_file}")
            if [ ${size} -gt 10 ]; then
                echo "  ✓ Configuration works with ${repo}"
            else
                echo "  ✗ Configuration with ${repo} produced no output"
                ((failed++))
            fi
        else
            echo "  ✗ Configuration failed with ${repo}"
            echo "  Error output:"
            cat "${output_file}"
            ((failed++))
        fi
        
        rm -f "${output_file}"
    done
    
    if [ ${failed} -eq 0 ]; then
        echo "✓ Configuration works with all repository types"
        return 0
    else
        echo "✗ Configuration failed with ${failed} repository types"
        return 1
    fi
}

test_config_overrides() {
    echo "Testing configuration parameter overrides..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local config_file="${FIXTURES_DIR}/test-repos/config-test/configs/minimal.yml"
    
    if [ ! -f "${config_file}" ]; then
        echo "✗ Config file not found: ${config_file}"
        return 1
    fi
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    # Test various flag combinations that might override config
    local test_cases=(
        "--verbose"
        "--quiet"
        "--quality-gate"
    )
    
    local failed=0
    for flag in "${test_cases[@]}"; do
        local output_file="/tmp/valknut_config_override_${flag//--/}_test.json"
        
        echo "  Testing config with ${flag}..."
        if "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config_file}" ${flag} --format json > "${output_file}" 2>&1; then
            local size=$(wc -c < "${output_file}")
            if [ ${size} -gt 10 ]; then
                echo "  ✓ Configuration with ${flag} works"
            else
                echo "  ✗ Configuration with ${flag} produced no output"
                ((failed++))
            fi
        else
            echo "  ✗ Configuration with ${flag} failed"
            if [ -f "${output_file}" ]; then
                echo "  Error output:"
                head -5 "${output_file}"
            fi
            ((failed++))
        fi
        
        rm -f "${output_file}"
    done
    
    if [ ${failed} -eq 0 ]; then
        echo "✓ Configuration parameter overrides test passed"
        return 0
    else
        echo "✗ ${failed} configuration override tests failed"
        return 1
    fi
}

# Run tests if called directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "Running configuration file tests..."
    
    setup_test_data
    
    failed_tests=0
    
    test_minimal_config || ((failed_tests++))
    test_maximum_config || ((failed_tests++))
    test_invalid_config || ((failed_tests++))
    test_nonexistent_config || ((failed_tests++))
    test_malformed_yaml_config || ((failed_tests++))
    test_empty_config || ((failed_tests++))
    test_config_with_different_repos || ((failed_tests++))
    test_config_overrides || ((failed_tests++))
    
    if [ ${failed_tests} -eq 0 ]; then
        echo "All configuration tests passed!"
    else
        echo "${failed_tests} configuration tests failed!"
        exit 1
    fi
fi