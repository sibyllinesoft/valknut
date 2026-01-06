//! Configuration for bundled JavaScript/TypeScript file detection.

use serde::{Deserialize, Serialize};

/// Configuration for detecting bundled JavaScript/TypeScript files.
///
/// Bundled files are detected by scanning for bundler runtime signatures
/// (webpack, rollup, esbuild, parcel) in the file header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundledDetectionConfig {
    /// Enable bundled file detection (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Maximum bytes to scan for bundler signatures (default: 4096)
    ///
    /// Bundler runtime code typically appears at the beginning of the file,
    /// so scanning the first 4KB is usually sufficient.
    #[serde(default = "default_scan_limit_bytes")]
    pub scan_limit_bytes: usize,
}

const fn default_enabled() -> bool {
    true
}

const fn default_scan_limit_bytes() -> usize {
    4096
}

impl Default for BundledDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            scan_limit_bytes: default_scan_limit_bytes(),
        }
    }
}
