# Attic - Quarantined Development Artifacts

This directory contains development artifacts that were moved from the project root during repository cleanup.

## Contents

- **configs/**: Alternative configuration files used during development
- **debug-files/**: Debug scripts, test output files, and temporary analysis results  
- **development-logs/**: Analysis logs and debugging output from development sessions
- **reports/**: Development reports and implementation summaries

## Restoration

These files were moved with `git mv` to preserve history. To restore any file:

```bash
git mv attic/[category]/[filename] ./[filename]
```

## Cleanup Schedule

Files in this directory will be reviewed for deletion after one release cycle unless they prove useful.