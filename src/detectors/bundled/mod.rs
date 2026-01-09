//! Bundled JavaScript/TypeScript file detection.
//!
//! This module detects bundled files by scanning for bundler runtime signatures
//! in the file header. Supported bundlers: webpack, rollup, esbuild, parcel.

mod config;

pub use config::BundledDetectionConfig;

use std::path::Path;

/// Bundler runtime signatures to detect.
///
/// Each pattern is a substring that indicates bundler-generated code.
/// These appear in the runtime/bootstrap code that bundlers inject.
const BUNDLER_SIGNATURES: &[&str] = &[
    // Webpack
    "__webpack_require__",
    "__webpack_exports__",
    "__webpack_modules__",
    "webpackJsonp",
    // esbuild
    "__toESM(",
    "__toCommonJS(",
    "__export(",
    "__commonJS(",
    // Parcel
    "parcelRequire",
    "parcelRegister",
    // Rollup (CommonJS interop)
    "Object.defineProperty(exports, '__esModule'",
    "Object.defineProperty(exports, \"__esModule\"",
];

/// File extensions to check for bundled content.
const BUNDLED_EXTENSIONS: &[&str] = &["js", "mjs", "cjs", "jsx", "ts", "tsx", "mts", "cts"];

/// Detects bundled JavaScript/TypeScript files by content analysis.
#[derive(Debug, Clone)]
pub struct BundledFileDetector {
    config: BundledDetectionConfig,
}

impl BundledFileDetector {
    /// Creates a new detector with the given configuration.
    pub fn new(config: BundledDetectionConfig) -> Self {
        Self { config }
    }

    /// Returns whether this detector is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Checks if the given file path has a JS/TS extension worth checking.
    pub fn should_check(&self, path: &Path) -> bool {
        if !self.config.enabled {
            return false;
        }

        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| BUNDLED_EXTENSIONS.contains(&ext))
            .unwrap_or(false)
    }

    /// Detects if the file content appears to be from a bundler.
    ///
    /// Scans the first `scan_limit_bytes` of content for bundler signatures.
    pub fn is_bundled(&self, content: &str) -> bool {
        if !self.config.enabled {
            return false;
        }

        // Only scan the header portion for performance
        // Find a valid UTF-8 character boundary at or before scan_limit_bytes
        let scan_content = if content.len() > self.config.scan_limit_bytes {
            let mut end = self.config.scan_limit_bytes;
            while end > 0 && !content.is_char_boundary(end) {
                end -= 1;
            }
            &content[..end]
        } else {
            content
        };

        BUNDLER_SIGNATURES
            .iter()
            .any(|sig| scan_content.contains(sig))
    }
}

impl Default for BundledFileDetector {
    fn default() -> Self {
        Self::new(BundledDetectionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webpack_detection() {
        let detector = BundledFileDetector::default();

        let webpack_content = r#"
/******/ (() => { // webpackBootstrap
/******/ 	var __webpack_modules__ = ({
/******/ 		"./src/index.js": ((__unused_webpack_module, __webpack_exports__, __webpack_require__) => {
"#;
        assert!(detector.is_bundled(webpack_content));
    }

    #[test]
    fn test_esbuild_detection() {
        let detector = BundledFileDetector::default();

        let esbuild_content = r#"
var __defProp = Object.defineProperty;
__export(target, all);
__toCommonJS(mod);
"#;
        assert!(detector.is_bundled(esbuild_content));
    }

    #[test]
    fn test_parcel_detection() {
        let detector = BundledFileDetector::default();

        let parcel_content = r#"
parcelRequire = (function (modules, cache, entry, globalName) {
  var previousRequire = typeof parcelRequire === 'function' && parcelRequire;
"#;
        assert!(detector.is_bundled(parcel_content));
    }

    #[test]
    fn test_rollup_detection() {
        let detector = BundledFileDetector::default();

        let rollup_content = r#"
'use strict';

Object.defineProperty(exports, '__esModule', { value: true });

var foo = require('./foo.js');
"#;
        assert!(detector.is_bundled(rollup_content));
    }

    #[test]
    fn test_normal_js_not_detected() {
        let detector = BundledFileDetector::default();

        let normal_content = r#"
export function hello() {
    console.log("Hello, world!");
}

export class Greeter {
    greet(name) {
        return `Hello, ${name}!`;
    }
}
"#;
        assert!(!detector.is_bundled(normal_content));
    }

    #[test]
    fn test_should_check_extensions() {
        let detector = BundledFileDetector::default();

        assert!(detector.should_check(Path::new("app.js")));
        assert!(detector.should_check(Path::new("app.ts")));
        assert!(detector.should_check(Path::new("app.tsx")));
        assert!(detector.should_check(Path::new("app.mjs")));
        assert!(!detector.should_check(Path::new("app.rs")));
        assert!(!detector.should_check(Path::new("app.py")));
    }

    #[test]
    fn test_disabled_detector() {
        let config = BundledDetectionConfig {
            enabled: false,
            ..Default::default()
        };
        let detector = BundledFileDetector::new(config);

        assert!(!detector.should_check(Path::new("app.js")));
        assert!(!detector.is_bundled("__webpack_require__"));
    }

    #[test]
    fn test_scan_limit() {
        let config = BundledDetectionConfig {
            enabled: true,
            scan_limit_bytes: 50,
        };
        let detector = BundledFileDetector::new(config);

        // Signature appears after scan limit (100 chars padding, then signature)
        let content = format!("{}{}", "x".repeat(100), "__webpack_require__");
        assert!(!detector.is_bundled(&content));

        // Signature within scan limit (signature first, then padding)
        let content2 = format!("{}{}", "__webpack_require__", "x".repeat(100));
        assert!(detector.is_bundled(&content2));
    }
}
