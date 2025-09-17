#!/bin/bash
# Comprehensive performance benchmarking script for Valknut
# Creates baseline performance metrics for regression testing

set -euo pipefail

echo "🚀 Starting Valknut Performance Benchmarking Suite"

# Configuration
BENCHMARK_DIR="benchmarks"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_FILE="$BENCHMARK_DIR/benchmark-$TIMESTAMP.json"
BINARY="./target/release/valknut"

# Ensure benchmark directory exists
mkdir -p "$BENCHMARK_DIR"

# Ensure release binary exists
if [ ! -f "$BINARY" ]; then
    echo "📦 Building release binary..."
    cargo build --release --all-features
fi

# System information
echo "💻 Collecting system information..."
SYSTEM_INFO=$(cat << EOF
{
  "system": {
    "os": "$(uname -s)",
    "arch": "$(uname -m)",
    "kernel": "$(uname -r)",
    "cpu_cores": $(nproc),
    "memory_gb": $(free -g | awk '/^Mem:/{print $2}'),
    "rustc_version": "$(rustc --version)",
    "binary_size_mb": $(stat --format="%s" "$BINARY" | awk '{print $1/1024/1024}')
  },
  "timestamp": "$TIMESTAMP",
  "benchmarks": []
}
EOF
)

# Function to run benchmark with timing
run_benchmark() {
    local test_name="$1"
    local test_path="$2"
    local args="$3"
    local description="$4"
    
    echo "🧪 Running benchmark: $test_name"
    echo "   Path: $test_path"
    echo "   Args: $args"
    
    # Count files first
    local file_count=0
    if [ -d "$test_path" ]; then
        file_count=$(find "$test_path" -type f \( -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.go" \) | wc -l)
    fi
    
    # Pre-analysis memory usage
    local mem_before=$(free -m | awk '/^Mem:/{print $3}')
    
    # Run analysis with timing
    local start_time=$(date +%s.%N)
    local output_file="$BENCHMARK_DIR/tmp-$test_name-$TIMESTAMP.json"
    
    # Capture both timing and memory
    /usr/bin/time -f "max_memory_mb:%M peak_memory_mb:%M" \
        "$BINARY" analyze "$test_path" \
        --format json \
        --out "$output_file" \
        $args 2> "$BENCHMARK_DIR/time-$test_name.tmp" || {
        echo "⚠️  Benchmark $test_name failed, continuing..."
        return 1
    }
    
    local end_time=$(date +%s.%N)
    local duration=$(echo "$end_time - $start_time" | bc)
    
    # Extract memory usage
    local max_memory=$(grep "max_memory_mb" "$BENCHMARK_DIR/time-$test_name.tmp" | cut -d: -f2 || echo "0")
    
    # Parse analysis results if available
    local entities_analyzed=0
    local issues_found=0
    local health_score=0
    
    if [ -f "$output_file" ]; then
        entities_analyzed=$(jq -r '.summary.entities_analyzed // 0' "$output_file" 2>/dev/null || echo "0")
        issues_found=$(jq -r '.refactoring_candidates | length' "$output_file" 2>/dev/null || echo "0")
        health_score=$(jq -r '.summary.code_health_score // 0' "$output_file" 2>/dev/null || echo "0")
    fi
    
    # Calculate performance metrics
    local files_per_second=$(echo "scale=2; $file_count / $duration" | bc)
    local entities_per_second=$(echo "scale=2; $entities_analyzed / $duration" | bc)
    
    # Create benchmark result
    local benchmark_result=$(cat << EOF
{
  "name": "$test_name",
  "description": "$description",
  "path": "$test_path",
  "args": "$args",
  "duration_seconds": $duration,
  "file_count": $file_count,
  "entities_analyzed": $entities_analyzed,
  "issues_found": $issues_found,
  "health_score": $health_score,
  "max_memory_mb": $max_memory,
  "performance": {
    "files_per_second": $files_per_second,
    "entities_per_second": $entities_per_second,
    "mb_per_file": $(echo "scale=4; $max_memory / $file_count" | bc | sed 's/^\./0./')
  }
}
EOF
    )
    
    # Add to results
    echo "$benchmark_result" >> "$BENCHMARK_DIR/results-$TIMESTAMP.jsonl"
    
    # Cleanup
    rm -f "$output_file" "$BENCHMARK_DIR/time-$test_name.tmp"
    
    echo "   ✅ Duration: ${duration}s | Files: $file_count | Memory: ${max_memory}MB"
}

# Initialize results file
echo "$SYSTEM_INFO" > "$RESULTS_FILE"

# Run benchmark suite
echo "🏃 Starting benchmark tests..."

# Small project benchmark - self analysis (core only)
run_benchmark "small_core" "src/core" "" "Small project: Valknut core module analysis"

# Medium project benchmark - detectors
run_benchmark "medium_detectors" "src/detectors" "--all-features" "Medium project: All detector modules with features"

# Large project benchmark - full self analysis
run_benchmark "large_full" "src" "--all-features" "Large project: Full Valknut self-analysis"

# Performance intensive - with clone detection
run_benchmark "intensive_clones" "src" "--all-features --semantic-clones" "Performance intensive: Full analysis with clone detection"

# Memory intensive - with oracle if available
if [ -n "${GEMINI_API_KEY:-}" ]; then
    run_benchmark "memory_oracle" "src/core" "--all-features --oracle" "Memory intensive: Analysis with AI oracle"
fi

# Compile results into final JSON
echo "📊 Compiling benchmark results..."

# Combine all results
{
    echo "{"
    echo "  \"system\": $(echo "$SYSTEM_INFO" | jq '.system'),"
    echo "  \"timestamp\": \"$TIMESTAMP\","
    echo "  \"benchmarks\": ["
    
    first=true
    while IFS= read -r line; do
        if [ "$first" = true ]; then
            first=false
        else
            echo ","
        fi
        echo "    $line"
    done < "$BENCHMARK_DIR/results-$TIMESTAMP.jsonl"
    
    echo "  ]"
    echo "}"
} > "$RESULTS_FILE"

# Generate summary report
echo "📈 Generating summary report..."

SUMMARY=$(cat << 'EOF'
#!/usr/bin/env python3
import json
import sys

with open(sys.argv[1]) as f:
    data = json.load(f)

print(f"🎯 Valknut Performance Benchmark Report")
print(f"📅 Date: {data['timestamp']}")
print(f"💻 System: {data['system']['os']} {data['system']['arch']}")
print(f"🧠 CPU Cores: {data['system']['cpu_cores']}")
print(f"🏠 Memory: {data['system']['memory_gb']}GB")
print(f"📦 Binary Size: {data['system']['binary_size_mb']:.1f}MB")
print()

benchmarks = data['benchmarks']
if not benchmarks:
    print("❌ No benchmark results found")
    sys.exit(1)

print("📊 Benchmark Results:")
print("-" * 80)
print(f"{'Test':<20} {'Duration':<10} {'Files':<8} {'Memory':<10} {'Files/s':<10}")
print("-" * 80)

for b in benchmarks:
    print(f"{b['name']:<20} {b['duration_seconds']:<10.2f} {b['file_count']:<8} {b['max_memory_mb']:<10} {b['performance']['files_per_second']:<10.1f}")

print("-" * 80)

# Calculate averages
total_duration = sum(b['duration_seconds'] for b in benchmarks)
total_files = sum(b['file_count'] for b in benchmarks)
avg_memory = sum(b['max_memory_mb'] for b in benchmarks) / len(benchmarks)
avg_files_per_sec = sum(b['performance']['files_per_second'] for b in benchmarks) / len(benchmarks)

print(f"{'AVERAGE':<20} {total_duration/len(benchmarks):<10.2f} {total_files//len(benchmarks):<8} {avg_memory:<10.1f} {avg_files_per_sec:<10.1f}")
print()

# Performance assessment
if avg_files_per_sec > 100:
    print("🚀 Performance: EXCELLENT (>100 files/sec)")
elif avg_files_per_sec > 50:
    print("✅ Performance: GOOD (50-100 files/sec)")
elif avg_files_per_sec > 20:
    print("⚠️  Performance: ACCEPTABLE (20-50 files/sec)")
else:
    print("❌ Performance: NEEDS IMPROVEMENT (<20 files/sec)")

if avg_memory < 100:
    print("🧠 Memory Usage: EXCELLENT (<100MB)")
elif avg_memory < 200:
    print("✅ Memory Usage: GOOD (100-200MB)")
elif avg_memory < 500:
    print("⚠️  Memory Usage: ACCEPTABLE (200-500MB)")
else:
    print("❌ Memory Usage: HIGH (>500MB)")

EOF
)

echo "$SUMMARY" > "$BENCHMARK_DIR/summarize.py"
chmod +x "$BENCHMARK_DIR/summarize.py"

# Run summary
python3 "$BENCHMARK_DIR/summarize.py" "$RESULTS_FILE"

# Cleanup temporary files
rm -f "$BENCHMARK_DIR/results-$TIMESTAMP.jsonl"

echo ""
echo "📁 Full results saved to: $RESULTS_FILE"
echo "📊 To view detailed results: jq . $RESULTS_FILE"
echo "🔄 To compare with previous runs: find $BENCHMARK_DIR -name 'benchmark-*.json' | sort"

# Save as latest baseline
cp "$RESULTS_FILE" "$BENCHMARK_DIR/baseline.json"
echo "💾 Saved as baseline for future comparisons"