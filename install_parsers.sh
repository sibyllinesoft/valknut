#!/bin/bash
# Tree-sitter Parser Installation Script for Valknut
# This script installs all required tree-sitter language parsers

set -euo pipefail

echo "🌳 Installing Tree-sitter Language Parsers for Valknut"
echo "================================================="

# Check if we're in a virtual environment
if [[ "${VIRTUAL_ENV:-}" == "" ]]; then
    echo "⚠️  Warning: Not in a virtual environment. Consider activating one first."
    echo "   Example: python3 -m venv venv && source venv/bin/activate"
    echo ""
fi

echo "📦 Installing core tree-sitter parsers..."

# Core language parsers for the enhanced valknut
parsers=(
    "tree-sitter-python>=0.20.0"
    "tree-sitter-javascript>=0.20.0" 
    "tree-sitter-typescript>=0.20.0"
    "tree-sitter-rust>=0.20.0"
    "tree-sitter-go>=0.20.0"
    "tree-sitter-bash>=0.20.0"
)

for parser in "${parsers[@]}"; do
    echo "  Installing $parser..."
    if pip install "$parser"; then
        echo "  ✅ $parser installed successfully"
    else
        echo "  ❌ Failed to install $parser"
        echo "  📝 Note: This parser may not be available. Valknut will gracefully handle missing parsers."
    fi
done

echo ""
echo "🔍 Verifying installation..."

# Test imports
python3 -c "
import sys

parsers = [
    ('tree_sitter_python', 'Python'),
    ('tree_sitter_javascript', 'JavaScript'),
    ('tree_sitter_typescript', 'TypeScript'),
    ('tree_sitter_rust', 'Rust'),
    ('tree_sitter_go', 'Go'),
    ('tree_sitter_bash', 'Bash')
]

available = []
unavailable = []

for module, name in parsers:
    try:
        __import__(module)
        available.append(name)
        print(f'✅ {name} parser available')
    except ImportError:
        unavailable.append(name)
        print(f'❌ {name} parser not available')

print()
print(f'📊 Summary: {len(available)}/{len(parsers)} parsers available')
print(f'✅ Available: {', '.join(available)}')
if unavailable:
    print(f'❌ Unavailable: {', '.join(unavailable)}')
print()
print('🎯 Valknut will use available parsers and gracefully handle missing ones.')
"

echo ""
echo "🎉 Parser installation complete!"
echo "🚀 You can now run: python3 -m valknut --help"
echo ""
echo "💡 If any parsers failed to install, Valknut will still work with available parsers."
echo "   Check individual parser documentation for installation troubleshooting."