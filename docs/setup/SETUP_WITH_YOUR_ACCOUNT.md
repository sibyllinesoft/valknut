# Setup Commands for githubcustomerserviceistrash Account

## 1. Authenticate with GitHub CLI

```bash
# Clear any existing tokens
unset GITHUB_TOKEN

# Login with your account
gh auth login
# Select: GitHub.com
# Select: HTTPS
# Select: Login with web browser
# This will open a browser - login as githubcustomerserviceistrash
```

## 2. Verify Authentication

```bash
gh auth status
# Should show: githubcustomerserviceistrash account
```

## 3. Create the Release

```bash
# Create and push tag
git tag -a v0.1.0 -m "Initial release - AI-powered code analysis tool"
git push origin v0.1.0

# Create GitHub release with binary
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
\`\`\`" \
  ./target/release/valknut
```

## 4. Create Homebrew Tap Repository

```bash
# Go to tap directory
cd ../homebrew-valknut

# Initialize if needed
git init
git add .
git commit -m "Initial Homebrew tap for Valknut"

# Create repository under sibyllinesoft organization
gh repo create sibyllinesoft/homebrew-valknut \
  --public \
  --description "Homebrew tap for Valknut - AI-powered code analysis tool" \
  --source=. \
  --remote=origin \
  --push
```

## 5. Update Formula with Release SHA256

```bash
# Get SHA256 of release tarball
SHA256=$(curl -sL https://github.com/sibyllinesoft/valknut/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256 | cut -d' ' -f1)
echo "SHA256: $SHA256"

# Update the formula
cat > Formula/valknut.rb << EOF
class Valknut < Formula
  desc "AI-powered code analysis and refactoring assistant"
  homepage "https://github.com/sibyllinesoft/valknut"
  url "https://github.com/sibyllinesoft/valknut/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "$SHA256"
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
git commit -m "Update formula with v0.1.0 release SHA256"
git push origin main
```

## 6. Test Installation

```bash
# Test the tap installation
brew tap sibyllinesoft/valknut
brew install valknut

# Verify it works
valknut --version
valknut analyze .
```

## All Commands in One Script

You can also run this automated script after authenticating:

```bash
cd /Users/nathan/Projects/valknut
./scripts/setup-github-homebrew.sh
```

That's it! Once you run these commands with your authenticated account, Valknut will be available via Homebrew.