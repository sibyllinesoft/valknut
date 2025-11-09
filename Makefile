# Valknut - Comprehensive Build and Test Makefile
# Supports all test targets, compilation, and development workflows

.PHONY: help build test test-unit test-cli test-e2e test-all bench clean install dev lint fmt check release docs doc-audit docker

DOC_AUDIT_FLAGS ?=

# Default target
help:
	@echo "Valknut Build and Test System"
	@echo "============================="
	@echo ""
	@echo "Build Targets:"
	@echo "  build         - Build in development mode"
	@echo "  release       - Build optimized release binary"
	@echo "  install       - Install valknut binary locally"
	@echo ""
	@echo "Test Targets:"
	@echo "  test          - Run all tests (unit + CLI + E2E)"
	@echo "  test-unit     - Run unit tests only"
	@echo "  test-cli      - Run CLI integration tests only"
	@echo "  test-e2e      - Run end-to-end CLI tests"
	@echo "  bench         - Run performance benchmarks"
	@echo ""
	@echo "Development:"
	@echo "  dev           - Development build with fast compile"
	@echo "  check         - Check code without building"
	@echo "  lint          - Run clippy linter"
	@echo "  fmt           - Format code with rustfmt"
	@echo "  clean         - Clean build artifacts"
	@echo ""
	@echo "Documentation:"
	@echo "  docs          - Generate and open documentation"
	@echo "  doc-audit     - Audit doc coverage and README freshness"
	@echo ""
	@echo "Feature Builds:"
	@echo "  build-simd    - Build with SIMD optimizations"
	@echo "  build-full    - Build with all features enabled"
	@echo "  build-minimal - Build with minimal features"
	@echo ""
	@echo "GitHub Actions Compatibility:"
	@echo "  gh-check      - Run GitHub Actions 'check' job locally"
	@echo "  gh-test       - Run GitHub Actions 'test' job locally"
	@echo "  gh-security   - Run security audit"
	@echo "  gh-benchmarks - Test benchmark compilation"
	@echo "  gh-cross      - Test cross-compilation setup"
	@echo "  gh-actions    - Run complete GitHub Actions simulation"
	@echo "  gh-quick      - Quick GitHub Actions checks (most common failures)"
	@echo ""
	@echo "Act Integration:"
	@echo "  act-check     - Run 'check' job with act"
	@echo "  act-test      - Run 'test' job with act"
	@echo "  act-all       - Run full CI with act"

# ==============================================================================
# Build Targets
# ==============================================================================

build:
	@echo "🔨 Building Valknut (development)..."
	cargo build

release:
	@echo "🚀 Building Valknut (release with optimizations)..."
	cargo build --release --features "simd,parallel,jemalloc"

dev: build
	@echo "✅ Development build complete"

install: release
	@echo "📦 Installing Valknut..."
	cargo install --path . --features "simd,parallel,jemalloc"

# Feature-specific builds
build-simd:
	@echo "⚡ Building with SIMD optimizations..."
	cargo build --release --features "simd,parallel"

build-full:
	@echo "🔧 Building with all features..."
	cargo build --release --all-features

build-minimal:
	@echo "📦 Building minimal version..."
	cargo build --release --no-default-features

# ==============================================================================
# Test Targets
# ==============================================================================

test: test-unit test-cli test-e2e
	@echo "✅ All tests completed successfully!"

test-unit:
	@echo "🧪 Running unit tests..."
	cargo test --lib --no-fail-fast

test-cli:
	@echo "🖥️  Running CLI integration tests..."
	cargo test --test cli_tests --no-fail-fast

test-e2e:
	@echo "🔄 Running CLI end-to-end tests..."
	@cd tests/cli-e2e-tests && ./run_e2e_tests.sh

test-all: test bench
	@echo "🎯 Complete test suite finished!"

# Performance testing
bench:
	@echo "⚡ Running performance benchmarks..."
	cargo bench --features benchmarks

BENCH_CLONE_PATH ?= .
bench-clone-verification:
	@echo "📊 Comparing clone verification quality (path: $(BENCH_CLONE_PATH))..."
	cargo run --release --example clone_verification -- "$(BENCH_CLONE_PATH)" baseline
	cargo run --release --example clone_verification -- "$(BENCH_CLONE_PATH)" apted

# Test with specific features
test-simd:
	@echo "🧮 Testing SIMD optimizations..."
	cargo test --features simd --no-fail-fast

test-parallel:
	@echo "⚡ Testing parallel processing..."
	cargo test --features parallel --no-fail-fast

# ==============================================================================
# Development and Quality
# ==============================================================================

check:
	@echo "🔍 Checking code..."
	cargo check --all-features

lint:
	@echo "🔍 Running clippy linter..."
	cargo clippy --all-features -- -D warnings

fmt:
	@echo "🎨 Formatting code..."
	cargo fmt --all

fmt-check:
	@echo "🔍 Checking code format..."
	cargo fmt --all -- --check

clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	@echo "🧹 Cleaning test artifacts..."
	@rm -rf tests/cli-e2e-tests/fixtures/test-repos/
	@rm -rf tests/cli-e2e-tests/test-output/
	@echo "✅ Clean complete"

# ==============================================================================
# Documentation
# ==============================================================================

docs:
	@echo "📖 Generating documentation..."
	cargo doc --all-features --open

docs-no-open:
	@echo "📖 Generating documentation..."
	cargo doc --all-features

doc-audit:
	@echo "Running documentation audit..."
	@cargo run -- doc-audit $(DOC_AUDIT_FLAGS)

# ==============================================================================
# Quality Assurance and CI/CD
# ==============================================================================

ci: fmt-check lint test-all
	@echo "🎯 CI pipeline completed successfully!"

pre-commit: fmt lint test-unit
	@echo "✅ Pre-commit checks passed!"

coverage:
	@echo "📊 Running test coverage analysis..."
	@if command -v cargo-tarpaulin >/dev/null 2>&1; then \
		cargo tarpaulin --all-features --out Html --output-dir coverage/; \
		echo "📊 Coverage report generated in coverage/tarpaulin-report.html"; \
	else \
		echo "❌ cargo-tarpaulin not installed. Run: cargo install cargo-tarpaulin"; \
	fi

# ==============================================================================
# GitHub Actions Compatibility Targets
# ==============================================================================

# Exact same check job as GitHub Actions
gh-check:
	@echo "🚀 Running GitHub Actions 'check' job locally..."
	@echo "📋 Setting GitHub Actions environment..."
	@export GITHUB_ACTIONS=true && \
	export CI=true && \
	export RUSTFLAGS="-D warnings" && \
	export CARGO_TERM_COLOR=always && \
	echo "🎨 Checking code formatting..." && \
	cargo fmt --all -- --check && \
	echo "🔍 Running clippy with GitHub Actions strictness..." && \
	cargo clippy --all-targets --all-features -- -D clippy::correctness -D clippy::suspicious -D clippy::complexity -W clippy::perf -W clippy::style && \
	echo "📚 Checking documentation..." && \
	cargo doc --all-features --no-deps --document-private-items && \
	echo "✅ GitHub Actions 'check' job completed successfully!"

# GitHub Actions test job equivalent
gh-test:
	@echo "🧪 Running GitHub Actions 'test' job locally..."
	@export GITHUB_ACTIONS=true && \
	export CI=true && \
	export RUSTFLAGS="-D warnings" && \
	export CARGO_TERM_COLOR=always && \
	echo "🧪 Running unit tests..." && \
	cargo test --lib && \
	echo "🔗 Running integration tests..." && \
	cargo test --tests && \
	echo "🎯 Running all feature tests..." && \
	cargo test --all-features && \
	echo "✅ GitHub Actions 'test' job completed successfully!"

# Security audit matching GitHub Actions
gh-security:
	@echo "🔒 Running security audit..."
	@export GITHUB_ACTIONS=true && \
	export CI=true && \
	if command -v cargo-audit >/dev/null 2>&1; then \
		cargo audit; \
	else \
		echo "📦 Installing cargo-audit..."; \
		cargo install cargo-audit; \
		cargo audit; \
	fi && \
	echo "✅ Security audit completed!"

# Benchmark compilation test
gh-benchmarks:
	@echo "⚡ Testing benchmark compilation..."
	@export GITHUB_ACTIONS=true && \
	export CI=true && \
	echo "🔍 Checking benchmark compilation..." && \
	cargo check --benches && \
	echo "🔍 Checking benchmarks with features..." && \
	cargo check --benches --features benchmarks && \
	echo "✅ Benchmark compilation test completed!"

# Cross-compilation simulation (native only)
gh-cross:
	@echo "🌐 Testing cross-compilation setup..."
	@export GITHUB_ACTIONS=true && \
	export CI=true && \
	echo "🎯 Installing cross-compilation targets..." && \
	rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true && \
	rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true && \
	echo "🔨 Testing release build..." && \
	cargo build --release && \
	echo "✅ Cross-compilation test completed!"

# Complete GitHub Actions simulation
gh-actions: gh-check gh-security gh-benchmarks gh-test gh-cross
	@echo "🎉 Complete GitHub Actions simulation completed successfully!"

# Quick GitHub Actions check (most common failures)
gh-quick:
	@echo "⚡ Running quick GitHub Actions checks..."
	@export GITHUB_ACTIONS=true && \
	export CI=true && \
	export RUSTFLAGS="-D warnings" && \
	cargo fmt --all -- --check && \
	cargo clippy --all-targets --all-features -- -D clippy::correctness -D clippy::suspicious -D clippy::complexity -W clippy::perf -W clippy::style && \
	cargo check --benches && \
	cargo audit && \
	echo "✅ Quick GitHub Actions checks completed!"

# Act runner targets
act-check:
	@echo "🎭 Running 'check' job with act..."
	@if command -v act >/dev/null 2>&1; then \
		act -j check; \
	else \
		echo "❌ act not installed. Install with:"; \
		echo "curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash"; \
		exit 1; \
	fi

act-test:
	@echo "🎭 Running 'test' job with act..."
	@if command -v act >/dev/null 2>&1; then \
		act -j test; \
	else \
		echo "❌ act not installed. Install with:"; \
		echo "curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash"; \
		exit 1; \
	fi

act-all:
	@echo "🎭 Running full CI with act..."
	@if command -v act >/dev/null 2>&1; then \
		act; \
	else \
		echo "❌ act not installed. Install with:"; \
		echo "curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash"; \
		exit 1; \
	fi

# ==============================================================================
# Development Environment
# ==============================================================================

setup-dev:
	@echo "🔧 Setting up development environment..."
	@rustup component add clippy rustfmt
	@if ! command -v cargo-tarpaulin >/dev/null 2>&1; then \
		echo "📊 Installing cargo-tarpaulin for coverage..."; \
		cargo install cargo-tarpaulin; \
	fi
	@if ! command -v cargo-watch >/dev/null 2>&1; then \
		echo "👀 Installing cargo-watch for development..."; \
		cargo install cargo-watch; \
	fi
	@echo "✅ Development environment ready!"

watch:
	@echo "👀 Watching for changes (running tests)..."
	cargo watch -x "test --lib"

watch-cli:
	@echo "👀 Watching for changes (CLI tests)..."
	cargo watch -x "test --test cli_tests"

# ==============================================================================
# Performance and Profiling
# ==============================================================================

profile:
	@echo "📊 Building for profiling..."
	cargo build --release --features "jemalloc,profiling"

flamegraph:
	@echo "🔥 Generating flamegraph (requires cargo-flamegraph)..."
	@if command -v cargo-flamegraph >/dev/null 2>&1; then \
		cargo flamegraph --bin valknut -- analyze ./src; \
	else \
		echo "❌ cargo-flamegraph not installed. Run: cargo install flamegraph"; \
	fi

# ==============================================================================
# Release Management
# ==============================================================================

release-check: ci
	@echo "🔍 Pre-release checks..."
	@cargo tree --duplicates
	@echo "✅ Release checks passed!"

tag-release:
	@echo "🏷️  Creating release tag..."
	@read -p "Enter version (e.g., v1.0.0): " version; \
	git tag -a $$version -m "Release $$version"; \
	echo "Created tag: $$version"

# ==============================================================================
# Docker Support
# ==============================================================================

docker-build:
	@echo "🐳 Building Docker image..."
	docker build -t valknut:latest .

docker-test:
	@echo "🐳 Testing in Docker container..."
	docker run --rm -v $(PWD):/workspace valknut:latest make test

# ==============================================================================
# Utility Targets
# ==============================================================================

size:
	@echo "📏 Binary size analysis..."
	@ls -lh target/release/valknut 2>/dev/null || echo "❌ Release binary not found. Run 'make release' first."

deps:
	@echo "📦 Dependency tree..."
	cargo tree

outdated:
	@echo "📦 Checking for outdated dependencies..."
	@if command -v cargo-outdated >/dev/null 2>&1; then \
		cargo outdated; \
	else \
		echo "❌ cargo-outdated not installed. Run: cargo install cargo-outdated"; \
	fi

# ==============================================================================
# Information
# ==============================================================================

info:
	@echo "Valknut Project Information"
	@echo "=========================="
	@echo "Rust version:    $$(rustc --version)"
	@echo "Cargo version:   $$(cargo --version)"
	@echo "Project status:  $$(git status --porcelain | wc -l) uncommitted changes"
	@echo "Last commit:     $$(git log -1 --format='%h %s' 2>/dev/null || echo 'No git repository')"
	@echo "Build features:  $$(grep '^default =' Cargo.toml | cut -d'=' -f2 || echo 'Default features')"
	@echo "Tests:           505 unit tests + 17 CLI tests + E2E suite"
