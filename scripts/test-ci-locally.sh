#!/bin/bash
# Local CI testing script that matches GitHub Actions strictness exactly
# Usage: ./scripts/test-ci-locally.sh [job_name]

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Set the same environment as GitHub Actions
export GITHUB_ACTIONS=true
export CI=true
export RUNNER_OS=Linux
export RUNNER_ARCH=X64
export RUSTFLAGS="-D warnings"
export CARGO_TERM_COLOR=always
export CARGO_INCREMENTAL=0
export CARGO_NET_RETRY=10
export CARGO_NET_TIMEOUT=60

# Ensure we're in the project root
cd "$(dirname "$0")/.."

print_status "Starting local CI testing with GitHub Actions strictness"

# Function to run the check job locally
run_check_job() {
    print_status "Running 'check' job (formatting, clippy, docs)"
    
    # Check formatting (same as GitHub Actions)
    print_status "Checking code formatting..."
    if cargo fmt --all -- --check; then
        print_success "Code formatting check passed"
    else
        print_error "Code formatting check failed"
        return 1
    fi
    
    # Run clippy with exact same flags as GitHub Actions
    print_status "Running clippy with GitHub Actions strictness..."
    if cargo clippy --all-targets --all-features -- -D clippy::correctness -D clippy::suspicious -D clippy::complexity -W clippy::perf -W clippy::style; then
        print_success "Clippy check passed"
    else
        print_error "Clippy check failed"
        return 1
    fi
    
    # Check docs
    print_status "Checking documentation..."
    if cargo doc --all-features --no-deps --document-private-items; then
        print_success "Documentation check passed"
    else
        print_error "Documentation check failed"
        return 1
    fi
    
    print_success "All 'check' job steps passed"
}

# Function to run security audit
run_security_audit() {
    print_status "Running security audit..."
    
    if command -v cargo-audit &> /dev/null; then
        if cargo audit; then
            print_success "Security audit passed"
        else
            print_error "Security audit failed"
            return 1
        fi
    else
        print_warning "cargo-audit not installed, installing..."
        cargo install cargo-audit
        if cargo audit; then
            print_success "Security audit passed"
        else
            print_error "Security audit failed"
            return 1
        fi
    fi
}

# Function to test benchmarks compilation
test_benchmarks() {
    print_status "Testing benchmark compilation..."
    
    if cargo check --benches; then
        print_success "Benchmark compilation passed"
    else
        print_error "Benchmark compilation failed"
        return 1
    fi
    
    # Test with benchmarks feature
    if cargo check --benches --features benchmarks; then
        print_success "Benchmark compilation with features passed"
    else
        print_error "Benchmark compilation with features failed"
        return 1
    fi
}

# Function to run tests
run_tests() {
    print_status "Running test suite..."
    
    # Unit tests
    print_status "Running unit tests..."
    if cargo test --lib; then
        print_success "Unit tests passed"
    else
        print_error "Unit tests failed"
        return 1
    fi
    
    # Integration tests
    print_status "Running integration tests..."
    if cargo test --tests; then
        print_success "Integration tests passed"
    else
        print_error "Integration tests failed"
        return 1
    fi
    
    # All features tests
    print_status "Running tests with all features..."
    if cargo test --all-features; then
        print_success "All features tests passed"
    else
        print_error "All features tests failed"
        return 1
    fi
}

# Function to test cross-compilation locally (simulated)
test_cross_compilation() {
    print_status "Testing cross-compilation setup..."
    
    # Check if cross-compilation targets are installed
    print_status "Checking Rust targets..."
    
    # Install targets that GitHub Actions uses
    local targets=("x86_64-unknown-linux-gnu" "x86_64-apple-darwin" "x86_64-pc-windows-msvc" "aarch64-unknown-linux-gnu")
    
    for target in "${targets[@]}"; do
        print_status "Installing target: $target"
        if rustup target add "$target"; then
            print_success "Target $target installed"
        else
            print_warning "Could not install target $target (may not be available on this platform)"
        fi
    done
    
    # Test compilation for native target
    print_status "Testing release build..."
    if cargo build --release; then
        print_success "Release build passed"
    else
        print_error "Release build failed"
        return 1
    fi
}

# Function to run act with specific job
run_with_act() {
    local job_name="$1"
    print_status "Running '$job_name' job with act..."
    
    if command -v act &> /dev/null; then
        if act -j "$job_name"; then
            print_success "Act job '$job_name' passed"
        else
            print_error "Act job '$job_name' failed"
            return 1
        fi
    else
        print_error "act not installed. Install with: curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash"
        return 1
    fi
}

# Main execution
main() {
    local job_name="${1:-all}"
    
    case "$job_name" in
        "check")
            run_check_job
            ;;
        "test")
            run_tests
            ;;
        "security")
            run_security_audit
            ;;
        "benchmarks")
            test_benchmarks
            ;;
        "cross")
            test_cross_compilation
            ;;
        "act-check")
            run_with_act "check"
            ;;
        "act-test")
            run_with_act "test"
            ;;
        "all")
            print_status "Running complete CI simulation"
            run_check_job
            run_security_audit
            test_benchmarks
            run_tests
            test_cross_compilation
            print_success "All local CI checks completed successfully"
            ;;
        *)
            echo "Usage: $0 [check|test|security|benchmarks|cross|act-check|act-test|all]"
            echo ""
            echo "Jobs:"
            echo "  check      - Run formatting, clippy, and docs checks"
            echo "  test       - Run test suite"
            echo "  security   - Run security audit"
            echo "  benchmarks - Test benchmark compilation"
            echo "  cross      - Test cross-compilation setup"
            echo "  act-check  - Run check job with act"
            echo "  act-test   - Run test job with act"
            echo "  all        - Run all checks (default)"
            exit 1
            ;;
    esac
}

# Execute main function with all arguments
main "$@"