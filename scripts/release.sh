#!/bin/bash
# Release script for Valknut

set -e

VERSION=${1:-}

if [ -z "$VERSION" ]; then
    echo "Usage: ./scripts/release.sh <version>"
    echo "Example: ./scripts/release.sh 0.1.0"
    exit 1
fi

echo "Creating release for Valknut v$VERSION"

# Update version in Cargo.toml
sed -i.bak "s/^version = .*/version = \"$VERSION\"/" Cargo.toml
rm Cargo.toml.bak

# Build release binary
echo "Building release binary..."
cargo build --release

# Create git tag
echo "Creating git tag v$VERSION..."
git add Cargo.toml Cargo.lock
git commit -m "Release v$VERSION" || true
git tag -a "v$VERSION" -m "Release v$VERSION"

echo "Release v$VERSION created!"
echo ""
echo "Next steps:"
echo "1. Push the tag: git push origin v$VERSION"
echo "2. Create a GitHub release with the tag"
echo "3. Upload the binary from target/release/valknut"
echo "4. Update the Homebrew formula with the release URL and SHA256"