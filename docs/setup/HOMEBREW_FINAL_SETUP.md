# Final Homebrew Setup Instructions

## Current Status
- ✅ Valknut successfully builds on macOS
- ✅ Binary created at `target/release/valknut` (1.99 MB)
- ✅ Homebrew formula created
- ✅ Homebrew tap directory structure created
- ✅ All documentation and scripts prepared
- ❌ Need GitHub permissions to complete setup

## Required Steps

### Option 1: If you have access to sibyllinesoft organization

1. **Switch to correct GitHub account:**
```bash
gh auth logout
gh auth login
# Login with account that has access to sibyllinesoft
```

2. **Run the automated setup:**
```bash
./scripts/setup-github-homebrew.sh
```

### Option 2: Manual setup with correct account

1. **Push the tag:**
```bash
git tag -a v0.1.0 -m "Initial release"
git push origin v0.1.0
```

2. **Create GitHub release:**
```bash
gh release create v0.1.0 \
  --title "Valknut v0.1.0" \
  --notes "Initial release of Valknut - AI-powered code analysis tool" \
  ./target/release/valknut
```

3. **Create tap repository:**
```bash
cd ../homebrew-valknut
git init
git add .
git commit -m "Initial Homebrew tap"

# Create repo under sibyllinesoft organization
gh repo create sibyllinesoft/homebrew-valknut --public --source=. --push
```

4. **Update formula with SHA256:**
```bash
# Get SHA256
SHA256=$(curl -sL https://github.com/sibyllinesoft/valknut/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256 | cut -d' ' -f1)

# Update formula
sed -i '' "s/PLACEHOLDER_SHA256/$SHA256/" Formula/valknut.rb

# Commit and push
git add Formula/valknut.rb
git commit -m "Update SHA256 for v0.1.0"
git push origin main
```

### Option 3: Fork to your own account

If you want to test with your own account first:

1. **Fork the repository:**
```bash
gh repo fork sibyllinesoft/valknut --clone=false
```

2. **Update remote:**
```bash
git remote add myfork https://github.com/YOUR_USERNAME/valknut
git push myfork v0.1.0
```

3. **Create your own tap:**
```bash
cd ../homebrew-valknut
# Update formula URLs to point to your fork
sed -i '' 's/sibyllinesoft/YOUR_USERNAME/g' Formula/valknut.rb

gh repo create YOUR_USERNAME/homebrew-valknut --public --source=. --push
```

## Testing the Installation

Once everything is set up:

```bash
# For sibyllinesoft tap:
brew tap sibyllinesoft/valknut
brew install valknut

# For your fork:
brew tap YOUR_USERNAME/valknut
brew install valknut

# Verify installation
valknut --version
valknut analyze .
```

## Files Created Summary

- `/target/release/valknut` - Built binary
- `/Formula/valknut.rb` - Formula in main repo
- `/scripts/release.sh` - Release automation
- `/scripts/setup-github-homebrew.sh` - GitHub setup automation
- `/HOMEBREW.md` - Comprehensive guide
- `/homebrew-valknut/` - Complete tap directory
  - `Formula/valknut.rb` - Homebrew formula
  - `README.md` - Tap documentation

Everything is ready - you just need to run the commands with an account that has the correct GitHub permissions!