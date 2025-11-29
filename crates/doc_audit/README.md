# doc_audit (Valknut crate)

Rust crate that powers Valknut’s documentation audit features: detecting missing/stale READMEs, docstring gaps, and directory doc health.

## What it does
- Scans source trees for doc coverage and freshness.
- Flags directories that exceed configurable complexity/size thresholds without accompanying docs.
- Emits structured results consumed by the Valknut CLI and reports.

## How to use
In the workspace it’s pulled in by `valknut-rs`; you can run its tests directly:

```bash
cargo test -p doc_audit
```

Key code lives in `src/lib.rs` with detectors in `src/scanners/`. Configuration mirrors the top-level CLI doc-audit options.

## Extending
- Add new language-specific doc scanners under `src/scanners/`.
- Keep coverage high; tests live alongside modules in the same crate.
