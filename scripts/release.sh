#!/usr/bin/env bash
# Automated release flow for Valknut.
#
# Usage:
#   ./scripts/release.sh <version>
#
# Example:
#   ./scripts/release.sh 1.3.0

set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"

VERSION=${1:-}
if [[ -z "$VERSION" ]]; then
  echo "Usage: $0 <version>" >&2
  exit 1
fi

if [[ ! $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Version must follow semver (e.g. 1.3.0)." >&2
  exit 1
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "The GitHub CLI (gh) is required." >&2
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "GitHub CLI is not authenticated. Run 'gh auth login' first." >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required to update package versions." >&2
  exit 1
fi

if ! cargo set-version --help >/dev/null 2>&1; then
  echo "cargo-edit is required (install with 'cargo install cargo-edit')." >&2
  exit 1
fi

if ! grep -q "## \[$VERSION\]" CHANGELOG.md; then
  echo "No changelog entry found for v$VERSION." >&2
  exit 1
fi

echo "🔧 Updating crate and extension versions to $VERSION"
cargo set-version --workspace "$VERSION"

EXT_VERSION_FILE="vscode-extension/package.json"
if [[ -f $EXT_VERSION_FILE ]]; then
  tmp=$(mktemp)
  jq --arg v "$VERSION" '.version = $v' "$EXT_VERSION_FILE" > "$tmp"
  mv "$tmp" "$EXT_VERSION_FILE"
fi

UI_PACKAGE="templates/assets/package.json"
if [[ -f $UI_PACKAGE ]]; then
  tmp=$(mktemp)
  jq --arg v "$VERSION" '.version = $v' "$UI_PACKAGE" > "$tmp"
  mv "$tmp" "$UI_PACKAGE"
fi

echo "📦 Building release binary"
cargo build --release

ARTIFACT_DIR="target/release"
BINARY_PATH="$ARTIFACT_DIR/valknut"
RELEASE_TARBALL="valknut-$VERSION-x86_64-unknown-linux-gnu.tar.gz"

if [[ ! -f $BINARY_PATH ]]; then
  echo "Release binary not found at $BINARY_PATH" >&2
  exit 1
fi

tar -czf "$RELEASE_TARBALL" -C "$ARTIFACT_DIR" valknut

CHANGELOG_SNIPPET=$(awk '/^## \['"$VERSION"'\]/{flag=1;next}/^## \[/{flag=0}flag' CHANGELOG.md)
NOTES_FILE=$(mktemp)
printf "## v%s\n\n%s\n" "$VERSION" "$CHANGELOG_SNIPPET" > "$NOTES_FILE"

TAG="v$VERSION"

echo "📝 Creating git tag $TAG"
git add Cargo.toml Cargo.lock
for version_file in "$EXT_VERSION_FILE" "$UI_PACKAGE"; do
  if [[ -f $version_file ]]; then
    git add "$version_file"
  fi
done
if ! git diff --cached --quiet; then
  git commit -m "Release $TAG"
fi
git tag -a "$TAG" -m "Release $TAG"

echo "🚀 Publishing GitHub release"
gh release create "$TAG" \
  "$RELEASE_TARBALL" \
  --title "Valknut $TAG" \
  --notes-file "$NOTES_FILE"

echo "✅ Release $TAG published."
echo "Next steps:" \
     "\n  • git push origin main $TAG" \
     "\n  • Verify the release assets on GitHub" \
     "\n  • Update downstream formulas/packages if necessary"

rm -f "$RELEASE_TARBALL" "$NOTES_FILE"
