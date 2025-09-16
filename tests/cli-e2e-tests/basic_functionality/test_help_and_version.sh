#!/bin/bash
# Basic CLI help and version tests

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
VALKNUT_BIN="${PROJECT_ROOT}/target/release/valknut"

test_help_command() {
    echo "Testing --help command..."
    
    local output_file="/tmp/valknut_help_test.txt"
    
    if "${VALKNUT_BIN}" --help > "${output_file}" 2>&1; then
        # Check for expected content
        if grep -q "Analyze your codebase" "${output_file}"; then
            echo "✓ Help command contains expected description"
        else
            echo "✗ Help command missing expected description"
            return 1
        fi
        
        if grep -q "Commands:" "${output_file}"; then
            echo "✓ Help command shows commands section"
        else
            echo "✗ Help command missing commands section"
            return 1
        fi
        
        if grep -q "analyze" "${output_file}"; then
            echo "✓ Help command shows analyze command"
        else
            echo "✗ Help command missing analyze command"
            return 1
        fi
        
        echo "✓ Help command test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Help command failed to execute"
        rm -f "${output_file}"
        return 1
    fi
}

test_version_command() {
    echo "Testing --version command..."
    
    local output_file="/tmp/valknut_version_test.txt"
    
    if "${VALKNUT_BIN}" --version > "${output_file}" 2>&1; then
        # Check for version number pattern
        if grep -qE "[0-9]+\.[0-9]+\.[0-9]+" "${output_file}"; then
            echo "✓ Version command shows semantic version"
        else
            echo "✗ Version command doesn't show semantic version"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Version command test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Version command failed to execute"
        rm -f "${output_file}"
        return 1
    fi
}

test_analyze_help() {
    echo "Testing analyze --help command..."
    
    local output_file="/tmp/valknut_analyze_help_test.txt"
    
    if "${VALKNUT_BIN}" analyze --help > "${output_file}" 2>&1; then
        # Check for expected content
        if grep -q "Analyze code repositories" "${output_file}"; then
            echo "✓ Analyze help contains expected description"
        else
            echo "✗ Analyze help missing expected description"
            return 1
        fi
        
        if grep -q "\[PATHS\]" "${output_file}"; then
            echo "✓ Analyze help shows paths argument"
        else
            echo "✗ Analyze help missing paths argument"
            return 1
        fi
        
        if grep -q -- "--format" "${output_file}"; then
            echo "✓ Analyze help shows format option"
        else
            echo "✗ Analyze help missing format option"
            return 1
        fi
        
        if grep -q -- "--config" "${output_file}"; then
            echo "✓ Analyze help shows config option"
        else
            echo "✗ Analyze help missing config option"
            return 1
        fi
        
        echo "✓ Analyze help command test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Analyze help command failed to execute"
        rm -f "${output_file}"
        return 1
    fi
}

test_invalid_command() {
    echo "Testing invalid command..."
    
    local output_file="/tmp/valknut_invalid_test.txt"
    
    if "${VALKNUT_BIN}" invalid-command > "${output_file}" 2>&1; then
        echo "✗ Invalid command should fail but succeeded"
        rm -f "${output_file}"
        return 1
    else
        # Check that it shows help or error message
        if grep -q "error\|Error\|help\|Help\|command" "${output_file}"; then
            echo "✓ Invalid command shows appropriate error message"
        else
            echo "✗ Invalid command error message unclear"
            cat "${output_file}"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Invalid command test passed"
        rm -f "${output_file}"
        return 0
    fi
}

# Run tests if called directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "Running basic CLI help and version tests..."
    
    test_help_command
    test_version_command
    test_analyze_help
    test_invalid_command
    
    echo "All basic CLI tests passed!"
fi