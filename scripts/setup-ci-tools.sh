#!/bin/bash
# Setup CI tools for local development and validation
# This script installs tools that mirror the CI environment

set -euo pipefail

echo "🔧 Setting up CI tools for Valknut development..."

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to install cargo tools
install_cargo_tool() {
    local tool=$1
    local binary=${2:-$tool}
    
    if command_exists "$binary"; then
        echo "✅ $tool already installed"
    else
        echo "📦 Installing $tool..."
        cargo install "$tool"
    fi
}

# Install Rust toolchain components
echo "🦀 Installing Rust toolchain components..."
rustup component add rustfmt clippy

# Install cargo tools
echo "📦 Installing cargo development tools..."
install_cargo_tool "cargo-nextest" "cargo-nextest"
install_cargo_tool "cargo-tarpaulin" "cargo-tarpaulin"
install_cargo_tool "cargo-audit" "cargo-audit"
install_cargo_tool "cargo-deny" "cargo-deny"
install_cargo_tool "cargo-watch" "cargo-watch"

# Check for system tools
echo "🔍 Checking system tools..."

# ShellCheck
if command_exists shellcheck; then
    echo "✅ shellcheck already installed"
else
    echo "📦 Installing shellcheck..."
    if command_exists apt-get; then
        sudo apt-get update && sudo apt-get install -y shellcheck
    elif command_exists brew; then
        brew install shellcheck
    elif command_exists dnf; then
        sudo dnf install -y ShellCheck
    else
        echo "⚠️  Please install shellcheck manually for your system"
    fi
fi

# Python tools
if command_exists python3; then
    echo "📦 Installing Python linting tools..."
    python3 -m pip install --user ruff mypy
else
    echo "⚠️  Python3 not found, skipping Python tools"
fi

# Create local development commands
echo "🛠️  Creating development helper scripts..."

cat > scripts/dev-test.sh << 'EOF'
#!/bin/bash
# Run tests with nextest for better output
set -euo pipefail

echo "🧪 Running unit tests with nextest..."
cargo nextest run --profile ci

echo "🔗 Running integration tests..."
cargo test --test '*'

echo "📊 Generating coverage report..."
cargo tarpaulin --all-features --out html --output-dir coverage/

echo "✅ All tests completed. Coverage report: coverage/tarpaulin-report.html"
EOF

cat > scripts/dev-lint.sh << 'EOF'
#!/bin/bash
# Run all linting checks locally
set -euo pipefail

echo "🎨 Checking Rust formatting..."
cargo fmt --all -- --check

echo "📎 Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "🐚 Linting shell scripts..."
find . -name "*.sh" -type f | xargs shellcheck -e SC2086,SC2002

if command -v ruff >/dev/null 2>&1; then
    echo "🐍 Linting Python files..."
    find . -name "*.py" -type f | xargs ruff check --fix
fi

echo "🔒 Running security audit..."
cargo audit

echo "🔍 Checking dependencies..."
cargo deny check

echo "✅ All linting checks completed!"
EOF

cat > scripts/dev-quality-gates.sh << 'EOF'
#!/bin/bash
# Run quality gates locally (like CI)
set -euo pipefail

echo "🏗️  Building release binary..."
cargo build --release --all-features

echo "🚪 Running quality gates on self..."
./target/release/valknut analyze \
    --quality-gate \
    --max-complexity 75 \
    --min-health 60 \
    --fail-on-issues \
    --format json \
    --out quality-report.json \
    ./src

echo "✅ Quality gates passed! Report saved to quality-report.json"
EOF

# Make scripts executable
chmod +x scripts/dev-*.sh

echo "🎉 CI tools setup completed!"
echo ""
echo "📋 Available development commands:"
echo "  • scripts/dev-test.sh      - Run tests with coverage"
echo "  • scripts/dev-lint.sh      - Run all linting checks" 
echo "  • scripts/dev-quality-gates.sh - Run quality gates"
echo ""
echo "🔄 Integration with cargo-watch:"
echo "  • cargo watch -x 'nextest run'           - Auto-run tests on changes"
echo "  • cargo watch -x 'clippy --all-targets' - Auto-lint on changes"
echo ""
echo "💡 Run 'scripts/dev-lint.sh' before committing to ensure CI passes!"