# Changelog

## [1.5.2] - 2026-06-26

- Fixed report hierarchy generation for root-level source files to avoid recursive self-parent traversal and stack overflows.
- Fixed emitted HTML report asset resolution so the Sibylline theme, React file tree bundle, local logo, and local animation assets load from their checked-in locations.
- Hardened the release script so optional package manifests are only staged when present.
