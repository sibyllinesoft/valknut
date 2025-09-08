# 🤖 Agent Usage Guide for Valknut

## ⚠️ Important for AI Agents

**Always use the pipx-installed version of valknut, NOT the system Python module.**

### ✅ Correct Usage for Agents:

```bash
# Use the pipx-installed binary directly
valknut analyze /path/to/code --format json --out results/

# Check language support first
valknut list-languages
```

### ❌ Avoid These Commands:

```bash
# DON'T use system Python module - parsers may not be available
python3 -m valknut analyze ...

# DON'T assume parsers are installed in system Python
pip install tree-sitter-python  # May fail with externally-managed-environment
```

## 🔍 Troubleshooting for Agents

### If you see "tree_sitter_python not available" errors:

1. **Check which valknut you're using:**
   ```bash
   which valknut  
   # Should return: /home/nathan/.local/bin/valknut
   ```

2. **Verify language support:**
   ```bash
   valknut list-languages
   # All core languages should show "✅ Full Support"
   ```

3. **If languages show issues, reinstall:**
   ```bash
   pipx install /media/nathan/Seagate\ Hub/Projects/valknut --force
   ```

### Expected Language Support:

When working properly, `valknut list-languages` should show:

```
🔤 Supported Programming Languages
   Found 6 supported languages

╭─────────────────┬──────────────┬─────────────────┬───────────────────────────╮
│ Language        │ Extension    │     Status      │ Features                  │
├─────────────────┼──────────────┼─────────────────┼───────────────────────────┤
│ Javascript      │ .js, .jsx    │ ✅ Full Support │ Full analysis, complexity │
│ Python          │ .py          │ ✅ Full Support │ Full analysis, refactor   │
│ Rust            │ .rs          │ ✅ Full Support │ Full analysis, memory     │
│ Typescript      │ .ts, .tsx    │ ✅ Full Support │ Full analysis, type check │
│ Go              │ .go          │ ✅ Full Support │ Full analysis, patterns   │
│ Bash            │ .sh          │ ✅ Full Support │ Shell script analysis     │
╰─────────────────┴──────────────┴─────────────────┴───────────────────────────╯
```

## 🎯 Quick Test Commands:

```bash
# Test installation
valknut --version

# Test on a known Python project (should work properly)
valknut analyze /media/nathan/Seagate\ Hub/Projects/skald --format json

# Check for any parser issues
valknut list-languages | grep -E "(❌|⚠️)"
```

## 📊 Understanding Analysis Results:

When analysis works correctly, you should see:
- **Files analyzed**: Should match the actual Python/JS/TS files in the project
- **Code entities**: Should extract functions, classes, methods (not just show "Unknown")  
- **Language breakdown**: Should identify the correct programming language
- **Processing time**: Should be fast (under a few seconds for most projects)

### Red Flags (Indicating Parser Issues):
- Language shows as "Unknown" in reports
- Very few entities extracted from a large codebase
- Only TypeScript files analyzed in a Python project
- "tree_sitter_python not available" warnings in verbose output

## 🔧 Resolution for Common Issues:

1. **Parser Not Available**: Use pipx-installed valknut
2. **Wrong Language Detected**: Ensure file extensions are recognized
3. **No Entities Extracted**: Check that project has analyzable code files
4. **Performance Issues**: Verify git-aware discovery is working (should be very fast)

The pipx installation includes all necessary tree-sitter parsers and should work reliably for agent automation.