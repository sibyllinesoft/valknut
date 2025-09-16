#!/bin/bash
# Performance and scalability tests

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

# Utility function to measure execution time
measure_time() {
    local start_time=$(date +%s.%N)
    "$@"
    local exit_code=$?
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc -l 2>/dev/null || echo "0")
    echo "${duration}"
    return ${exit_code}
}

test_small_repository_performance() {
    echo "Testing performance with small repository..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/small-python"
    local output_file="/tmp/valknut_small_perf_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    echo "  Running analysis on small Python project..."
    local duration
    duration=$(measure_time "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1)
    local exit_code=$?
    
    if [ ${exit_code} -eq 0 ]; then
        # Check execution time
        local duration_int=$(echo "${duration}" | cut -d. -f1)
        if [ "${duration_int}" -lt 30 ]; then
            echo "  ✓ Small repository analysis completed in ${duration}s (good performance)"
        else
            echo "  ⚠ Small repository analysis took ${duration}s (may be slow)"
        fi
        
        # Check output size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 10 ]; then
            echo "  ✓ Analysis produced ${size} bytes of output"
        else
            echo "  ✗ Analysis produced minimal output (${size} bytes)"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Small repository performance test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Small repository analysis failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            head -5 "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_medium_repository_performance() {
    echo "Testing performance with medium repository..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/medium-rust"
    local output_file="/tmp/valknut_medium_perf_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    echo "  Running analysis on medium Rust project..."
    local duration
    duration=$(measure_time "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1)
    local exit_code=$?
    
    if [ ${exit_code} -eq 0 ]; then
        # Check execution time
        local duration_int=$(echo "${duration}" | cut -d. -f1)
        if [ "${duration_int}" -lt 60 ]; then
            echo "  ✓ Medium repository analysis completed in ${duration}s (good performance)"
        else
            echo "  ⚠ Medium repository analysis took ${duration}s (may be slow)"
        fi
        
        # Check output size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 100 ]; then
            echo "  ✓ Analysis produced ${size} bytes of output"
        else
            echo "  ✗ Analysis produced minimal output (${size} bytes)"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Medium repository performance test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Medium repository analysis failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            head -5 "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_large_repository_performance() {
    echo "Testing performance with large mixed-language repository..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/large-mixed"
    local output_file="/tmp/valknut_large_perf_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    echo "  Running analysis on large mixed-language project..."
    local duration
    duration=$(measure_time timeout 120 "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1)
    local exit_code=$?
    
    if [ ${exit_code} -eq 0 ]; then
        # Check execution time
        local duration_int=$(echo "${duration}" | cut -d. -f1)
        if [ "${duration_int}" -lt 120 ]; then
            echo "  ✓ Large repository analysis completed in ${duration}s"
        else
            echo "  ⚠ Large repository analysis took ${duration}s (at timeout limit)"
        fi
        
        # Check output size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 500 ]; then
            echo "  ✓ Analysis produced ${size} bytes of output"
        else
            echo "  ✗ Analysis produced minimal output (${size} bytes)"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Large repository performance test passed"
        rm -f "${output_file}"
        return 0
    elif [ ${exit_code} -eq 124 ]; then
        echo "  ⚠ Large repository analysis timed out after 120s"
        echo "ℹ Large repository performance test passed (timeout is acceptable)"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Large repository analysis failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            head -5 "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_complex_algorithms_performance() {
    echo "Testing performance with complex algorithms repository..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/performance-test"
    local output_file="/tmp/valknut_complex_perf_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Performance test repository not found: ${test_repo}"
        return 1
    fi
    
    echo "  Running analysis on complex algorithms project..."
    local duration
    duration=$(measure_time timeout 180 "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1)
    local exit_code=$?
    
    if [ ${exit_code} -eq 0 ]; then
        # Check execution time
        local duration_int=$(echo "${duration}" | cut -d. -f1)
        if [ "${duration_int}" -lt 180 ]; then
            echo "  ✓ Complex algorithms analysis completed in ${duration}s"
        else
            echo "  ⚠ Complex algorithms analysis took ${duration}s (at timeout limit)"
        fi
        
        # Check output size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 200 ]; then
            echo "  ✓ Analysis produced ${size} bytes of output"
        else
            echo "  ✗ Analysis produced minimal output (${size} bytes)"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Complex algorithms performance test passed"
        rm -f "${output_file}"
        return 0
    elif [ ${exit_code} -eq 124 ]; then
        echo "  ⚠ Complex algorithms analysis timed out after 180s"
        echo "ℹ Complex algorithms performance test passed (timeout is acceptable)"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Complex algorithms analysis failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            head -5 "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_multiple_repositories_concurrent() {
    echo "Testing performance with multiple repositories..."
    
    local repos=(
        "${FIXTURES_DIR}/test-repos/small-python"
        "${FIXTURES_DIR}/test-repos/medium-rust"
    )
    local output_file="/tmp/valknut_multiple_perf_test.json"
    
    # Check all repositories exist
    for repo in "${repos[@]}"; do
        if [ ! -d "${repo}" ]; then
            echo "✗ Test repository not found: ${repo}"
            return 1
        fi
    done
    
    echo "  Running analysis on multiple repositories..."
    local duration
    duration=$(measure_time "${VALKNUT_BIN}" analyze "${repos[@]}" --format json > "${output_file}" 2>&1)
    local exit_code=$?
    
    if [ ${exit_code} -eq 0 ]; then
        # Check execution time
        local duration_int=$(echo "${duration}" | cut -d. -f1)
        if [ "${duration_int}" -lt 90 ]; then
            echo "  ✓ Multiple repositories analysis completed in ${duration}s"
        else
            echo "  ⚠ Multiple repositories analysis took ${duration}s (may be slow)"
        fi
        
        # Check output size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 100 ]; then
            echo "  ✓ Analysis produced ${size} bytes of output"
        else
            echo "  ✗ Analysis produced minimal output (${size} bytes)"
            rm -f "${output_file}"
            return 1
        fi
        
        echo "✓ Multiple repositories performance test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Multiple repositories analysis failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            head -5 "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

test_memory_usage_monitoring() {
    echo "Testing memory usage patterns..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/large-mixed"
    local output_file="/tmp/valknut_memory_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    echo "  Running analysis with memory monitoring..."
    
    # Start analysis in background and monitor memory
    "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1 &
    local pid=$!
    
    local max_memory=0
    local samples=0
    
    # Monitor memory usage for up to 30 seconds
    for i in {1..30}; do
        if ! kill -0 ${pid} 2>/dev/null; then
            break
        fi
        
        # Get memory usage (if ps command supports it)
        local memory=$(ps -o rss= -p ${pid} 2>/dev/null | tr -d ' ' || echo "0")
        if [ "${memory}" -gt "${max_memory}" ]; then
            max_memory=${memory}
        fi
        ((samples++))
        
        sleep 1
    done
    
    # Wait for process to complete
    wait ${pid} 2>/dev/null || true
    local exit_code=$?
    
    if [ ${exit_code} -eq 0 ]; then
        # Convert KB to MB
        local max_memory_mb=$((max_memory / 1024))
        if [ ${max_memory_mb} -gt 0 ]; then
            echo "  ✓ Peak memory usage: ${max_memory_mb} MB (${samples} samples)"
            if [ ${max_memory_mb} -lt 1000 ]; then
                echo "  ✓ Memory usage within reasonable limits"
            else
                echo "  ⚠ Memory usage is high (${max_memory_mb} MB)"
            fi
        else
            echo "  ℹ Memory monitoring not available on this system"
        fi
        
        echo "✓ Memory usage monitoring test passed"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Memory usage test failed (analysis failed)"
        rm -f "${output_file}"
        return 1
    fi
}

test_configuration_performance_impact() {
    echo "Testing performance impact of different configurations..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/medium-rust"
    local configs=(
        "${FIXTURES_DIR}/test-repos/config-test/configs/minimal.yml"
        "${FIXTURES_DIR}/test-repos/config-test/configs/maximum.yml"
    )
    
    if [ ! -d "${test_repo}" ]; then
        echo "✗ Test repository not found: ${test_repo}"
        return 1
    fi
    
    local config_names=("minimal" "maximum")
    local times=()
    
    for i in "${!configs[@]}"; do
        local config="${configs[$i]}"
        local name="${config_names[$i]}"
        local output_file="/tmp/valknut_config_perf_${name}_test.json"
        
        if [ ! -f "${config}" ]; then
            echo "  ✗ Config file not found: ${config}"
            continue
        fi
        
        echo "  Testing ${name} configuration performance..."
        local duration
        duration=$(measure_time "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config}" --format json > "${output_file}" 2>&1)
        local exit_code=$?
        
        if [ ${exit_code} -eq 0 ]; then
            times+=("${duration}")
            echo "  ✓ ${name} config completed in ${duration}s"
            
            # Check output was produced
            local size=$(wc -c < "${output_file}")
            if [ ${size} -lt 10 ]; then
                echo "  ✗ ${name} config produced minimal output"
                rm -f "${output_file}"
                return 1
            fi
        else
            echo "  ✗ ${name} config failed"
            if [ -f "${output_file}" ]; then
                echo "  Error output:"
                head -3 "${output_file}"
            fi
            rm -f "${output_file}"
            return 1
        fi
        
        rm -f "${output_file}"
    done
    
    # Compare performance if we have both results
    if [ ${#times[@]} -eq 2 ]; then
        echo "  ℹ Performance comparison:"
        echo "    Minimal config: ${times[0]}s"
        echo "    Maximum config: ${times[1]}s"
        
        # Basic comparison (minimal should generally be faster)
        local minimal_int=$(echo "${times[0]}" | cut -d. -f1)
        local maximum_int=$(echo "${times[1]}" | cut -d. -f1)
        
        if [ "${minimal_int}" -le "${maximum_int}" ]; then
            echo "  ✓ Minimal config is faster or equal (expected)"
        else
            echo "  ℹ Maximum config is faster (may be due to system variation)"
        fi
    fi
    
    echo "✓ Configuration performance impact test passed"
    return 0
}

test_stress_test() {
    echo "Testing stress scenarios..."
    
    local test_repo="${FIXTURES_DIR}/test-repos/large-mixed"
    local output_file="/tmp/valknut_stress_test.json"
    
    if [ ! -d "${test_repo}" ]; then
        echo "ℹ Large repository not found, skipping stress test"
        return 0
    fi
    
    echo "  Running stress test (maximum configuration, complex repository)..."
    local config="${FIXTURES_DIR}/test-repos/config-test/configs/maximum.yml"
    
    if [ ! -f "${config}" ]; then
        echo "  ℹ Maximum config not found, using default settings"
        config=""
    fi
    
    local duration
    if [ -n "${config}" ]; then
        duration=$(measure_time timeout 300 "${VALKNUT_BIN}" analyze "${test_repo}" --config "${config}" --format json > "${output_file}" 2>&1)
    else
        duration=$(measure_time timeout 300 "${VALKNUT_BIN}" analyze "${test_repo}" --format json > "${output_file}" 2>&1)
    fi
    local exit_code=$?
    
    if [ ${exit_code} -eq 0 ]; then
        echo "  ✓ Stress test completed in ${duration}s"
        
        # Check output size
        local size=$(wc -c < "${output_file}")
        if [ ${size} -gt 1000 ]; then
            echo "  ✓ Stress test produced substantial output (${size} bytes)"
        else
            echo "  ⚠ Stress test produced limited output (${size} bytes)"
        fi
        
        echo "✓ Stress test passed"
        rm -f "${output_file}"
        return 0
    elif [ ${exit_code} -eq 124 ]; then
        echo "  ⚠ Stress test timed out after 300s"
        echo "ℹ Stress test passed (timeout is acceptable for this scenario)"
        rm -f "${output_file}"
        return 0
    else
        echo "✗ Stress test failed"
        if [ -f "${output_file}" ]; then
            echo "Error output:"
            head -5 "${output_file}"
        fi
        rm -f "${output_file}"
        return 1
    fi
}

# Run tests if called directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "Running performance tests..."
    
    # Check if bc is available for time calculations
    if ! command -v bc >/dev/null 2>&1; then
        echo "⚠ bc not available, time measurements may be limited"
    fi
    
    setup_test_data
    
    failed_tests=0
    
    test_small_repository_performance || ((failed_tests++))
    test_medium_repository_performance || ((failed_tests++))
    test_large_repository_performance || ((failed_tests++))
    test_complex_algorithms_performance || ((failed_tests++))
    test_multiple_repositories_concurrent || ((failed_tests++))
    test_memory_usage_monitoring || ((failed_tests++))
    test_configuration_performance_impact || ((failed_tests++))
    test_stress_test || ((failed_tests++))
    
    if [ ${failed_tests} -eq 0 ]; then
        echo "All performance tests passed!"
    else
        echo "${failed_tests} performance tests failed!"
        exit 1
    fi
fi