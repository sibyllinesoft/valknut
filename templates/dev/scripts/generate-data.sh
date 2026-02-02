#!/usr/bin/env bash
# Generate dev template data by running valknut on itself
# This script should be run from the repository root

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
DEV_DIR="$SCRIPT_DIR/.."

cd "$ROOT_DIR"

echo "Generating dev template data..."

# Build valknut if needed
if [[ ! -f "target/release/valknut" ]]; then
    echo "Building valknut (release)..."
    cargo build --release --quiet
fi

# Generate HTML report
echo "Running analysis..."
target/release/valknut analyze . \
    --format html \
    --out "$DEV_DIR/public/report-dev.html" \
    --profile balanced \
    2>/dev/null || true

# Also generate JSON for direct use
target/release/valknut analyze . \
    --format json \
    --out "$DEV_DIR/data/analysis.json" \
    --profile balanced \
    2>/dev/null || true

# Extract tree data from HTML
echo "Extracting tree data..."
cd "$DEV_DIR"
if command -v node &> /dev/null; then
    node scripts/extract-data.cjs
elif command -v bun &> /dev/null; then
    bun scripts/extract-data.cjs
else
    echo "Warning: Node.js or Bun required for extract-data.cjs"
fi

echo "Done! Generated:"
echo "  - $DEV_DIR/public/report-dev.html"
echo "  - $DEV_DIR/data/analysis.json"
echo "  - $DEV_DIR/data/tree-data.json"
echo "  - $DEV_DIR/public/data.json"
