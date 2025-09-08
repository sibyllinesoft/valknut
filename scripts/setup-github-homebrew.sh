#!/bin/bash
# GitHub and Homebrew setup script for Valknut
# Requires: GitHub CLI (gh) or a personal access token

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Valknut GitHub & Homebrew Setup Script${NC}"
echo "======================================"

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
    echo -e "${YELLOW}GitHub CLI (gh) is not installed.${NC}"
    echo "Install it with: brew install gh"
    echo "Or use the manual steps below."
    exit 1
fi

# Check if authenticated
if ! gh auth status &> /dev/null; then
    echo -e "${YELLOW}Not authenticated with GitHub.${NC}"
    echo "Run: gh auth login"
    exit 1
fi

# Function to create release
create_release() {
    echo -e "${GREEN}Creating GitHub release...${NC}"
    
    # Create tag if it doesn't exist
    if ! git tag | grep -q "v0.1.0"; then
        echo "Creating tag v0.1.0..."
        git tag -a v0.1.0 -m "Initial release - AI-powered code analysis tool"
        git push origin v0.1.0
    fi
    
    # Create release with the binary
    echo "Creating GitHub release..."
    gh release create v0.1.0 \
        --title "Valknut v0.1.0" \
        --notes "Initial release of Valknut - AI-powered code analysis and refactoring assistant.

## Features
- Comprehensive code analysis
- Technical debt assessment
- Refactoring recommendations
- Multi-language support (Python, Rust, TypeScript, JavaScript, Go)
- CI/CD integration with quality gates

## Installation

### Homebrew (macOS)
\`\`\`bash
brew tap sibyllinesoft/valknut
brew install valknut
\`\`\`

### From Source
\`\`\`bash
cargo build --release
\`\`\`

## Usage
\`\`\`bash
valknut analyze .
valknut --help
\`\`\`
" \
        ./target/release/valknut
    
    echo -e "${GREEN}Release created successfully!${NC}"
}

# Function to create homebrew tap repository
create_homebrew_tap() {
    echo -e "${GREEN}Creating Homebrew tap repository...${NC}"
    
    cd ../homebrew-valknut
    
    # Initialize git if needed
    if [ ! -d .git ]; then
        git init
        git add .
        git commit -m "Initial Homebrew tap for Valknut"
    fi
    
    # Create the repository on GitHub
    echo "Creating repository sibyllinesoft/homebrew-valknut..."
    gh repo create sibyllinesoft/homebrew-valknut \
        --public \
        --description "Homebrew tap for Valknut - AI-powered code analysis tool" \
        --source=. \
        --remote=origin \
        --push
    
    echo -e "${GREEN}Homebrew tap repository created!${NC}"
    cd ../valknut
}

# Function to update formula with release info
update_formula() {
    echo -e "${GREEN}Updating Homebrew formula...${NC}"
    
    # Get the tarball URL
    TARBALL_URL="https://github.com/sibyllinesoft/valknut/archive/refs/tags/v0.1.0.tar.gz"
    
    # Download and calculate SHA256
    echo "Calculating SHA256..."
    SHA256=$(curl -sL "$TARBALL_URL" | shasum -a 256 | cut -d' ' -f1)
    
    # Update the formula
    cd ../homebrew-valknut
    
    # Create updated formula
    cat > Formula/valknut.rb << EOF
class Valknut < Formula
  desc "AI-powered code analysis and refactoring assistant"
  homepage "https://github.com/sibyllinesoft/valknut"
  url "${TARBALL_URL}"
  sha256 "${SHA256}"
  license "MIT"
  head "https://github.com/sibyllinesoft/valknut.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "--locked"
    bin.install "target/release/valknut"
  end

  test do
    assert_match "valknut", shell_output("#{bin}/valknut --version")
    
    # Test help command
    assert_match "Analyze your codebase", shell_output("#{bin}/valknut --help")
    
    # Test list-languages command
    output = shell_output("#{bin}/valknut list-languages")
    assert_match "Python", output
    assert_match "Rust", output
  end
end
EOF

    # Commit and push the update
    git add Formula/valknut.rb
    git commit -m "Update formula with v0.1.0 release"
    git push origin main
    
    echo -e "${GREEN}Formula updated with release information!${NC}"
    cd ../valknut
}

# Main execution
echo ""
echo "This script will:"
echo "1. Create a GitHub release for v0.1.0"
echo "2. Create the homebrew-valknut tap repository"
echo "3. Update the formula with the release SHA256"
echo ""
read -p "Continue? (y/n) " -n 1 -r
echo ""

if [[ $REPLY =~ ^[Yy]$ ]]; then
    create_release
    create_homebrew_tap
    update_formula
    
    echo ""
    echo -e "${GREEN}Setup complete!${NC}"
    echo ""
    echo "Users can now install Valknut with:"
    echo "  brew tap sibyllinesoft/valknut"
    echo "  brew install valknut"
else
    echo -e "${YELLOW}Setup cancelled.${NC}"
fi

# Manual steps if gh CLI is not available
echo ""
echo "Manual steps (if needed):"
echo "1. Create release: https://github.com/sibyllinesoft/valknut/releases/new"
echo "2. Create tap repo: https://github.com/new (name: homebrew-valknut)"
echo "3. Update formula with SHA256 from: curl -sL https://github.com/sibyllinesoft/valknut/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256"