# 🔧 Valknut Installation Guide

## ⚡ Quick Start (Recommended)

**For most users, use pipx for isolated installation:**

```bash
# Install valknut with all dependencies
pipx install /media/nathan/Seagate\ Hub/Projects/valknut

# Verify installation
valknut --version
valknut list-languages
```

This automatically handles all tree-sitter parsers and creates an isolated environment.

## 🌳 Tree-sitter Parser Status

Valknut requires tree-sitter parsers for full language support:

| Language | Parser Package | Status |
|----------|----------------|---------|
| Python | `tree-sitter-python` | ✅ Available |
| JavaScript | `tree-sitter-javascript` | ✅ Available |  
| TypeScript | `tree-sitter-typescript` | ✅ Available |
| Rust | `tree-sitter-rust` | ✅ Available |
| Go | `tree-sitter-go` | ✅ Available |
| Bash | `tree-sitter-bash` | ✅ Available |

## 🛠️ Development Installation

For development work:

```bash
# Clone and install in development mode
cd /media/nathan/Seagate\ Hub/Projects/valknut
uv sync
uv run valknut --version
```

## 🐍 System Python Issues

If you see "externally-managed-environment" errors, this is normal for modern Python installations. Use pipx instead:

```bash
# ❌ This might fail with externally-managed-environment
pip install /media/nathan/Seagate\ Hub/Projects/valknut

# ✅ This works properly
pipx install /media/nathan/Seagate\ Hub/Projects/valknut
```

## 🔍 Troubleshooting

### "tree_sitter_python not available" errors

1. **Check if using pipx-installed version:**
   ```bash
   which valknut  # Should show /home/nathan/.local/bin/valknut
   ```

2. **If using system Python, switch to pipx:**
   ```bash
   pipx install /media/nathan/Seagate\ Hub/Projects/valknut --force
   ```

3. **Verify parser availability:**
   ```bash
   valknut list-languages
   ```

### Language Support Not Working

If you see "Unknown" language in reports:

1. **Check that parsers are available:**
   ```bash
   valknut list-languages
   ```

2. **Reinstall with pipx if needed:**
   ```bash
   pipx uninstall valknut
   pipx install /media/nathan/Seagate\ Hub/Projects/valknut
   ```

## 📚 For Agents and Automation

**Always use the pipx-installed version:**

```bash
# ✅ Recommended for agents
valknut analyze /path/to/code --format json --out results/

# ❌ Avoid system Python module calls
python3 -m valknut analyze ...
```

The pipx installation ensures all tree-sitter parsers are available and working correctly.

## 🎯 Verification Commands

After installation, verify everything works:

```bash
# Check version and help
valknut --version
valknut --help

# Verify language support
valknut list-languages

# Test analysis on a small project
valknut analyze /media/nathan/Seagate\ Hub/Projects/skald --format markdown
```

All languages should show "✅ Full Support" status for optimal analysis results.