# GitHub Setup Commands for Valknut Homebrew Distribution

Run these commands to set up Valknut for Homebrew distribution:

## 1. Install GitHub CLI (if not already installed)
```bash
brew install gh
gh auth login
```

## 2. Create the GitHub Release
```bash
# In the valknut directory
cd /Users/nathan/Projects/valknut

# Create and push the tag
git tag -a v0.1.0 -m "Initial release - AI-powered code analysis tool"
git push origin v0.1.0

# Create the release with the binary
gh release create v0.1.0 \
  --title "Valknut v0.1.0" \
  --notes "Initial release of Valknut - AI-powered code analysis and refactoring assistant." \
  ./target/release/valknut
```

## 3. Create the Homebrew Tap Repository
```bash
# Go to the homebrew tap directory
cd /Users/nathan/Projects/homebrew-valknut

# Initialize git
git init
git add .
git commit -m "Initial Homebrew tap for Valknut"

# Create the repository on GitHub
gh repo create sibyllinesoft/homebrew-valknut \
  --public \
  --description "Homebrew tap for Valknut - AI-powered code analysis tool" \
  --source=. \
  --remote=origin \
  --push
```

## 4. Update Formula with Release SHA256
```bash
# Get the SHA256 of the release tarball
SHA256=$(curl -sL https://github.com/sibyllinesoft/valknut/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256 | cut -d' ' -f1)
echo "SHA256: $SHA256"

# Update the formula (you'll need to manually edit the sha256 line in the formula)
# Or use this command to do it automatically:
sed -i '' "s/sha256 \".*\"/sha256 \"$SHA256\"/" Formula/valknut.rb

# Commit and push the update
git add Formula/valknut.rb
git commit -m "Update formula with v0.1.0 release SHA256"
git push origin main
```

## 5. Test the Installation
```bash
# Test installing from your tap
brew tap sibyllinesoft/valknut
brew install valknut
valknut --version
```

## Alternative: Using Personal Access Token

If you prefer using a personal access token instead of gh CLI:

1. Go to https://github.com/settings/tokens
2. Generate a new token with `repo` scope
3. Use curl commands:

```bash
# Create release
curl -X POST \
  -H "Authorization: token YOUR_GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.v3+json" \
  https://api.github.com/repos/sibyllinesoft/valknut/releases \
  -d '{
    "tag_name": "v0.1.0",
    "name": "Valknut v0.1.0",
    "body": "Initial release of Valknut",
    "draft": false,
    "prerelease": false
  }'

# Create tap repository
curl -X POST \
  -H "Authorization: token YOUR_GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.v3+json" \
  https://api.github.com/orgs/sibyllinesoft/repos \
  -d '{
    "name": "homebrew-valknut",
    "description": "Homebrew tap for Valknut",
    "private": false
  }'
```

## Automated Setup

Run the provided script to automate all these steps:
```bash
./scripts/setup-github-homebrew.sh
```

This will handle all the GitHub operations automatically if you have gh CLI installed and authenticated.