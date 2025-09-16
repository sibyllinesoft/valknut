#!/bin/bash
# Development Environment Setup Script for Valknut
# This script sets up a complete development environment with all necessary tools

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check system requirements
check_system() {
    log_info "Checking system requirements..."
    
    # Check OS
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        OS="linux"
        log_success "Linux detected"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macos"
        log_success "macOS detected"
    else
        log_error "Unsupported operating system: $OSTYPE"
        exit 1
    fi
    
    # Check architecture
    ARCH=$(uname -m)
    log_info "Architecture: $ARCH"
    
    # Check for required system tools
    local required_tools=("curl" "git")
    for tool in "${required_tools[@]}"; do
        if command_exists "$tool"; then
            log_success "$tool is installed"
        else
            log_error "$tool is required but not installed"
            exit 1
        fi
    done
}

# Install Rust and Cargo tools
install_rust() {
    log_info "Setting up Rust toolchain..."
    
    if command_exists "rustc"; then
        local rust_version=$(rustc --version)
        log_success "Rust already installed: $rust_version"
    else
        log_info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source ~/.cargo/env
        log_success "Rust installed successfully"
    fi
    
    # Update Rust to latest stable
    log_info "Updating Rust to latest stable..."
    rustup update stable
    
    # Install required components
    log_info "Installing Rust components..."
    rustup component add clippy rustfmt
    
    # Install cargo tools needed for development
    log_info "Installing development cargo tools..."
    local cargo_tools=(
        "cargo-audit"           # Security auditing
        "cargo-deny"            # License and dependency checking
        "cargo-tarpaulin"       # Code coverage
        "cargo-criterion"       # Benchmarking
        "cargo-machete"         # Unused dependency detection
        "cargo-geiger"          # Unsafe code detection
        "cargo-license"         # License checking
        "cargo-outdated"        # Outdated dependency detection
        "cargo-edit"            # Dependency management (cargo add, cargo rm)
        "cargo-expand"          # Macro expansion
        "cargo-tree"            # Dependency tree visualization
    )
    
    for tool in "${cargo_tools[@]}"; do
        if cargo install --list | grep -q "^$tool "; then
            log_success "$tool already installed"
        else
            log_info "Installing $tool..."
            cargo install "$tool" || log_warning "Failed to install $tool (continuing...)"
        fi
    done
}

# Install system dependencies for Valknut
install_system_deps() {
    log_info "Installing system dependencies..."
    
    if [[ "$OS" == "linux" ]]; then
        # Check for package manager
        if command_exists "apt-get"; then
            log_info "Using apt-get for package installation..."
            sudo apt-get update
            sudo apt-get install -y \
                build-essential \
                pkg-config \
                libssl-dev \
                tree-sitter-cli \
                valgrind \
                perf-tools-unstable \
                linux-perf \
                bc \
                jq
        elif command_exists "yum"; then
            log_info "Using yum for package installation..."
            sudo yum groupinstall -y "Development Tools"
            sudo yum install -y \
                openssl-devel \
                tree-sitter \
                valgrind \
                perf \
                bc \
                jq
        elif command_exists "pacman"; then
            log_info "Using pacman for package installation..."
            sudo pacman -S --needed \
                base-devel \
                openssl \
                tree-sitter \
                valgrind \
                perf \
                bc \
                jq
        else
            log_warning "No supported package manager found (apt, yum, pacman)"
            log_warning "Please install build tools, openssl-dev, tree-sitter manually"
        fi
    elif [[ "$OS" == "macos" ]]; then
        if command_exists "brew"; then
            log_info "Using Homebrew for package installation..."
            brew install \
                openssl \
                tree-sitter \
                jq
        else
            log_warning "Homebrew not found. Please install: openssl, tree-sitter, jq"
        fi
    fi
    
    log_success "System dependencies installation completed"
}

# Setup pre-commit hooks
setup_precommit() {
    log_info "Setting up pre-commit hooks..."
    
    # Install pre-commit if not available
    if command_exists "pre-commit"; then
        log_success "pre-commit already installed"
    else
        log_info "Installing pre-commit..."
        if command_exists "pip3"; then
            pip3 install pre-commit
        elif command_exists "pip"; then
            pip install pre-commit
        elif [[ "$OS" == "macos" ]] && command_exists "brew"; then
            brew install pre-commit
        else
            log_error "Cannot install pre-commit. Please install Python/pip or Homebrew"
            return 1
        fi
    fi
    
    # Install pre-commit hooks
    if [[ -f ".pre-commit-config.yaml" ]]; then
        log_info "Installing pre-commit hooks..."
        pre-commit install
        log_success "Pre-commit hooks installed"
        
        # Run pre-commit on all files to test
        log_info "Testing pre-commit setup..."
        pre-commit run --all-files || log_warning "Some pre-commit checks failed (this is normal on first run)"
    else
        log_warning "No .pre-commit-config.yaml found, skipping pre-commit setup"
    fi
}

# Configure Git settings for development
setup_git() {
    log_info "Configuring Git for development..."
    
    # Check if user has configured Git
    if ! git config --get user.name >/dev/null; then
        log_warning "Git user.name not configured. Please run:"
        log_warning "  git config --global user.name 'Your Name'"
    fi
    
    if ! git config --get user.email >/dev/null; then
        log_warning "Git user.email not configured. Please run:"
        log_warning "  git config --global user.email 'your.email@example.com'"
    fi
    
    # Set up useful Git aliases for Valknut development
    git config --local alias.st status
    git config --local alias.br branch
    git config --local alias.co checkout
    git config --local alias.cm commit
    git config --local alias.lg "log --oneline --graph --decorate"
    
    log_success "Git configuration completed"
}

# Setup VS Code configuration (if VS Code is installed)
setup_vscode() {
    if command_exists "code"; then
        log_info "Setting up VS Code configuration..."
        
        mkdir -p .vscode
        
        # VS Code settings for Rust development
        cat > .vscode/settings.json << 'EOF'
{
    "rust-analyzer.check.command": "clippy",
    "rust-analyzer.check.allTargets": true,
    "rust-analyzer.check.features": "all",
    "rust-analyzer.cargo.features": "all",
    "rust-analyzer.procMacro.enable": true,
    "rust-analyzer.imports.granularity.group": "module",
    "rust-analyzer.completion.addCallArgumentSnippets": true,
    "rust-analyzer.completion.addCallParenthesis": true,
    "rust-analyzer.inlayHints.enable": true,
    "rust-analyzer.inlayHints.chainingHints": true,
    "rust-analyzer.inlayHints.parameterHints": true,
    "rust-analyzer.inlayHints.typeHints": true,
    "editor.formatOnSave": true,
    "editor.codeActionsOnSave": {
        "source.fixAll": true
    },
    "files.exclude": {
        "**/target": true,
        "**/.git": true
    },
    "search.exclude": {
        "**/target": true
    }
}
EOF

        # Recommended extensions
        cat > .vscode/extensions.json << 'EOF'
{
    "recommendations": [
        "rust-lang.rust-analyzer",
        "vadimcn.vscode-lldb",
        "serayuzgur.crates",
        "tamasfe.even-better-toml",
        "ms-vscode.test-adapter-converter",
        "hbenl.vscode-test-explorer",
        "streetsidesoftware.code-spell-checker"
    ]
}
EOF

        # Launch configuration for debugging
        cat > .vscode/launch.json << 'EOF'
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug executable 'valknut'",
            "cargo": {
                "args": [
                    "build",
                    "--bin=valknut",
                    "--package=valknut-rs"
                ],
                "filter": {
                    "name": "valknut",
                    "kind": "bin"
                }
            },
            "args": ["analyze", "tests/fixtures/"],
            "cwd": "${workspaceFolder}"
        },
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug unit tests",
            "cargo": {
                "args": [
                    "test",
                    "--no-run",
                    "--lib",
                    "--package=valknut-rs"
                ],
                "filter": {
                    "name": "valknut-rs",
                    "kind": "lib"
                }
            },
            "args": [],
            "cwd": "${workspaceFolder}"
        }
    ]
}
EOF

        log_success "VS Code configuration created"
    else
        log_info "VS Code not found, skipping VS Code setup"
    fi
}

# Validate the development environment
validate_environment() {
    log_info "Validating development environment..."
    
    # Check Rust installation
    if command_exists "rustc" && command_exists "cargo"; then
        local rust_version=$(rustc --version)
        log_success "Rust: $rust_version"
    else
        log_error "Rust installation validation failed"
        return 1
    fi
    
    # Test cargo build
    log_info "Testing cargo build..."
    if cargo check --all-targets --all-features; then
        log_success "Cargo build check passed"
    else
        log_error "Cargo build check failed"
        return 1
    fi
    
    # Test basic functionality
    log_info "Testing basic functionality..."
    if cargo test --lib -- --test-threads=1 --nocapture | head -20; then
        log_success "Basic tests passed"
    else
        log_warning "Some tests failed (this may be normal during development)"
    fi
    
    # Check security audit
    log_info "Running security audit..."
    if cargo audit; then
        log_success "Security audit passed"
    else
        log_warning "Security audit found issues (review and address as needed)"
    fi
    
    log_success "Development environment validation completed"
}

# Print development workflow information
print_workflow_info() {
    echo ""
    log_info "Development Environment Setup Complete!"
    echo ""
    echo "📋 Next Steps:"
    echo "  1. Run tests: cargo test"
    echo "  2. Run benchmarks: cargo bench --features benchmarks"
    echo "  3. Check code quality: cargo clippy --all-targets --all-features"
    echo "  4. Format code: cargo fmt"
    echo "  5. Security audit: cargo audit"
    echo ""
    echo "🔄 Pre-commit hooks are installed and will run automatically on commit"
    echo ""
    echo "🛠️  Available cargo tools:"
    echo "  • cargo audit          - Security vulnerability scanning"
    echo "  • cargo deny           - License and dependency policy enforcement"
    echo "  • cargo tarpaulin      - Code coverage analysis"
    echo "  • cargo geiger         - Unsafe code detection"
    echo "  • cargo machete        - Unused dependency detection"
    echo "  • cargo outdated       - Check for outdated dependencies"
    echo "  • cargo edit           - Add/remove dependencies (cargo add, cargo rm)"
    echo ""
    echo "📖 Documentation:"
    echo "  • Generate docs: cargo doc --open"
    echo "  • View README: cat README.md"
    echo ""
    echo "🚀 Ready for development!"
}

# Main execution
main() {
    echo "🔧 Valknut Development Environment Setup"
    echo "========================================"
    
    check_system
    install_rust
    install_system_deps
    setup_precommit
    setup_git
    setup_vscode
    validate_environment
    print_workflow_info
}

# Run main function
main "$@"