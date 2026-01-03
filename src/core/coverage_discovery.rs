//! Coverage file discovery utilities.
//!
//! This module provides utilities for discovering and detecting coverage files
//! in various formats (LCOV, Cobertura, JaCoCo, Istanbul, Tarpaulin, etc.).

use crate::core::config::CoverageConfig;
use crate::core::errors::{Result, ValknutError};
use crate::core::file_utils::FileReader;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{debug, info};

/// Coverage file discovery information
#[derive(Debug, Clone)]
pub struct CoverageFile {
    /// Path to the coverage file
    pub path: PathBuf,
    /// Detected format of the coverage file
    pub format: CoverageFormat,
    /// Last modified time
    pub modified: SystemTime,
    /// File size in bytes
    pub size: u64,
}

/// Coverage file format detection
#[derive(Debug, Clone, PartialEq)]
pub enum CoverageFormat {
    CoveragePyXml, // coverage.py XML format
    Lcov,          // LCOV .info format
    Cobertura,     // Cobertura XML format
    JaCoCo,        // JaCoCo XML format
    IstanbulJson,  // Istanbul JSON format
    Tarpaulin,     // Tarpaulin JSON format (Rust coverage)
    Unknown,
}

/// Detection and parsing methods for [`CoverageFormat`].
impl CoverageFormat {
    /// Detect format from file path and content
    pub fn detect(file_path: &Path) -> Result<Self> {
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // First try to detect by filename
        if filename.contains("coverage") && filename.ends_with(".xml") {
            return Ok(Self::CoveragePyXml);
        }

        if filename.ends_with("lcov.info") || filename == "lcov.info" || filename.ends_with(".lcov")
        {
            return Ok(Self::Lcov);
        }

        if filename.contains("cobertura") && filename.ends_with(".xml") {
            return Ok(Self::Cobertura);
        }

        if filename.ends_with(".json") {
            // For JSON files, need to check content to distinguish Istanbul vs Tarpaulin
            return Self::detect_json_by_content(file_path);
        }

        // If filename detection fails, try content-based detection
        Self::detect_by_content(file_path)
    }

    /// Detect format by examining file content
    fn detect_by_content(file_path: &Path) -> Result<Self> {
        if FileReader::is_likely_binary(file_path)? {
            return Ok(Self::Unknown);
        }

        // Read first few lines to detect format
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            ValknutError::io("Failed to read coverage file for format detection", e)
        })?;

        let first_kb = content
            .chars()
            .take(1024)
            .collect::<String>()
            .to_lowercase();

        if first_kb.contains("<?xml") {
            return Ok(Self::detect_xml_format(&first_kb));
        }

        if first_kb.starts_with("tn:")
            || first_kb.contains("\ntn:")
            || first_kb.starts_with("sf:")
            || first_kb.contains("\nsf:")
        {
            Ok(Self::Lcov)
        } else if first_kb.starts_with("{") && first_kb.contains("\"path\"") {
            Ok(Self::IstanbulJson)
        } else {
            Ok(Self::Unknown)
        }
    }

    /// Detect JSON coverage format by examining file content.
    /// Distinguishes between Istanbul and Tarpaulin formats.
    fn detect_json_by_content(file_path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(file_path).map_err(|e| {
            ValknutError::io("Failed to read coverage file for format detection", e)
        })?;

        // Only need to check the first ~1KB for structure indicators
        let first_kb: String = content.chars().take(1024).collect();

        // Tarpaulin has "files":[{"path":[ - path as array is distinctive
        // Istanbul has "path":"string" (path as string)
        if first_kb.contains("\"files\"") && first_kb.contains("\"path\":[") {
            return Ok(Self::Tarpaulin);
        }

        // Also check a larger window for files where the structure might be delayed
        let first_64kb: String = content.chars().take(65536).collect();
        if first_64kb.contains("\"files\"") && first_64kb.contains("\"traces\"") {
            return Ok(Self::Tarpaulin);
        }

        // Default to Istanbul for other JSON files
        Ok(Self::IstanbulJson)
    }

    /// Detect XML coverage format from content prefix.
    fn detect_xml_format(first_kb: &str) -> Self {
        if first_kb.contains("coverage") && first_kb.contains("branch-rate") {
            Self::Cobertura
        } else if first_kb.contains("coverage") {
            Self::CoveragePyXml
        } else if first_kb.contains("report") && first_kb.contains("package") {
            Self::JaCoCo
        } else {
            Self::Unknown
        }
    }
}

/// Coverage file discovery utility
pub struct CoverageDiscovery;

impl CoverageDiscovery {
    /// Discover coverage files across multiple roots and return a de-duplicated, recency-sorted list.
    pub fn discover_coverage_for_roots<T: AsRef<Path>>(
        roots: &[T],
        config: &CoverageConfig,
    ) -> Result<Vec<CoverageFile>> {
        let mut all = Vec::new();
        for root in roots {
            let mut found = Self::discover_coverage_files(root.as_ref(), config)?;
            all.append(&mut found);
        }
        all.sort_by(|a, b| b.modified.cmp(&a.modified));
        all.dedup_by(|a, b| a.path == b.path);
        Ok(all)
    }

    /// Discover coverage files in the given root path using configuration
    pub fn discover_coverage_files(
        root_path: &Path,
        config: &CoverageConfig,
    ) -> Result<Vec<CoverageFile>> {
        debug!(
            "Coverage discovery called with root_path: {}, coverage_file: {:?}, auto_discover: {}",
            root_path.display(),
            config.coverage_file,
            config.auto_discover
        );

        if let Some(ref explicit_file) = config.coverage_file {
            debug!("Using explicit coverage file: {}", explicit_file.display());
            // Use explicitly specified coverage file
            return Self::validate_coverage_file(explicit_file);
        }

        if !config.auto_discover {
            return Ok(Vec::new());
        }

        debug!(
            "Starting coverage file discovery in: {}",
            root_path.display()
        );

        let mut discovered_files = Vec::new();
        let max_age = if config.max_age_days > 0 {
            Some(Duration::from_secs(
                config.max_age_days as u64 * 24 * 60 * 60,
            ))
        } else {
            None
        };

        // Search each configured path
        for search_path in &config.search_paths {
            let full_path = root_path.join(search_path);
            if !full_path.exists() {
                debug!("Search path does not exist: {}", full_path.display());
                continue;
            }

            debug!("Searching for coverage files in: {}", full_path.display());

            // Search for files matching patterns
            for pattern in &config.file_patterns {
                let found_files = Self::find_files_by_pattern(&full_path, pattern, max_age)?;
                discovered_files.extend(found_files);
            }
        }

        // Sort by modification time (most recent first)
        discovered_files.sort_by(|a, b| b.modified.cmp(&a.modified));

        // Remove duplicates (same path)
        discovered_files.dedup_by(|a, b| a.path == b.path);

        info!("Discovered {} coverage files", discovered_files.len());
        for file in &discovered_files {
            info!(
                "  Found: {} (format: {:?}, size: {} bytes)",
                file.path.display(),
                file.format,
                file.size
            );
        }

        Ok(discovered_files)
    }

    /// Find files matching a specific pattern with enhanced discovery
    fn find_files_by_pattern(
        search_path: &Path,
        pattern: &str,
        max_age: Option<Duration>,
    ) -> Result<Vec<CoverageFile>> {
        let mut files = Vec::new();

        if pattern.contains("*") {
            // Use glob matching with multiple strategies
            let glob_patterns = Self::expand_glob_pattern(search_path, pattern);
            for glob_pattern in glob_patterns {
                Self::collect_glob_matches(&glob_pattern, max_age, &mut files);
            }
        } else {
            // Direct file lookup with intelligent fallbacks
            let candidate_paths = Self::expand_direct_pattern(search_path, pattern);
            for file_path in candidate_paths {
                Self::try_add_coverage_file(&file_path, max_age, &mut files);
            }
        }

        Ok(files)
    }

    /// Collect coverage files matching a glob pattern.
    fn collect_glob_matches(
        glob_pattern: &str,
        max_age: Option<Duration>,
        files: &mut Vec<CoverageFile>,
    ) {
        let Ok(paths) = glob::glob(glob_pattern) else {
            debug!("Glob pattern failed: {}", glob_pattern);
            return;
        };
        for entry in paths.flatten() {
            Self::try_add_coverage_file(&entry, max_age, files);
        }
    }

    /// Try to validate and add a coverage file to the collection.
    fn try_add_coverage_file(
        path: &Path,
        max_age: Option<Duration>,
        files: &mut Vec<CoverageFile>,
    ) {
        if let Ok(Some(file)) = Self::validate_coverage_file_with_age(path, max_age) {
            files.push(file);
        }
    }

    /// Expand glob pattern into multiple search strategies
    pub fn expand_glob_pattern(search_path: &Path, pattern: &str) -> Vec<String> {
        let mut patterns = Vec::new();
        let base_path = search_path.display().to_string();

        if pattern.starts_with("**/") {
            // Recursive pattern - search in all subdirectories
            patterns.push(format!("{}/{}", base_path, pattern));
            // Also try without leading **/ in immediate subdirectories
            let simple_pattern = &pattern[3..]; // Remove "*/"
            patterns.push(format!("{}/**/{}", base_path, simple_pattern));
        } else if pattern.contains("/") {
            // Path-based pattern - respect directory structure
            patterns.push(format!("{}/{}", base_path, pattern));
        } else {
            // Simple filename pattern - search recursively
            patterns.push(format!("{}/**/{}", base_path, pattern));
            // Also search in immediate directory
            patterns.push(format!("{}/{}", base_path, pattern));
        }

        patterns
    }

    /// Fallback paths for common coverage file patterns.
    pub const COVERAGE_FALLBACKS: &[(&str, &[&str])] = &[
        ("coverage.xml", &[
            "coverage/coverage.xml",
            "target/coverage/coverage.xml",
            "target/tarpaulin/coverage.xml",
            "test-results/coverage.xml",
            "reports/coverage.xml",
        ]),
        ("lcov.info", &[
            "coverage/lcov.info",
            "coverage-reports/lcov.info",
            "target/coverage/lcov.info",
        ]),
        ("coverage.json", &[
            "coverage/coverage-final.json",
            "coverage/coverage.json",
            "reports/coverage.json",
        ]),
    ];

    /// Expand direct pattern into intelligent fallback paths
    pub fn expand_direct_pattern(search_path: &Path, pattern: &str) -> Vec<PathBuf> {
        let mut paths = vec![search_path.join(pattern)];

        // Add fallback paths from lookup table
        if let Some((_, fallbacks)) = Self::COVERAGE_FALLBACKS.iter().find(|(p, _)| *p == pattern) {
            paths.extend(fallbacks.iter().map(|f| search_path.join(f)));
        }

        paths
    }

    /// Validate a coverage file and return CoverageFile if valid
    fn validate_coverage_file(file_path: &Path) -> Result<Vec<CoverageFile>> {
        match Self::validate_coverage_file_with_age(file_path, None)? {
            Some(file) => Ok(vec![file]),
            None => Ok(Vec::new()),
        }
    }

    /// Validate a coverage file with age check
    fn validate_coverage_file_with_age(
        file_path: &Path,
        max_age: Option<Duration>,
    ) -> Result<Option<CoverageFile>> {
        if !file_path.exists() {
            return Ok(None);
        }

        let metadata = fs::metadata(file_path)
            .map_err(|e| ValknutError::io("Failed to read file metadata", e))?;

        if !metadata.is_file() {
            return Ok(None);
        }

        let modified = metadata
            .modified()
            .map_err(|e| ValknutError::io("Failed to get file modification time", e))?;

        // Check age if specified
        if let Some(max_age) = max_age {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed > max_age {
                    debug!(
                        "Coverage file too old: {} (age: {:?})",
                        file_path.display(),
                        elapsed
                    );
                    return Ok(None);
                }
            }
        }

        // Detect format
        let format = CoverageFormat::detect(file_path).unwrap_or(CoverageFormat::Unknown);

        if matches!(format, CoverageFormat::Unknown) {
            debug!("Unknown coverage format: {}", file_path.display());
            return Ok(None);
        }

        Ok(Some(CoverageFile {
            path: file_path.to_path_buf(),
            format,
            modified,
            size: metadata.len(),
        }))
    }

    /// Get the most recent coverage file from discovered files
    pub fn get_most_recent(files: &[CoverageFile]) -> Option<&CoverageFile> {
        files.first() // Already sorted by modification time (most recent first)
    }

    /// Filter coverage files by format
    pub fn filter_by_format(files: &[CoverageFile], format: CoverageFormat) -> Vec<&CoverageFile> {
        files.iter().filter(|f| f.format == format).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_coverage_format_detection_by_filename_and_content() {
        let temp_dir = TempDir::new().unwrap();
        let lcov = temp_dir.path().join("lcov.info");
        fs::write(&lcov, "TN:\nSF:src/lib.rs\nDA:1,1\nend_of_record\n").unwrap();
        assert_eq!(CoverageFormat::detect(&lcov).unwrap(), CoverageFormat::Lcov);

        let cobertura = temp_dir.path().join("cobertura.xml");
        fs::write(
            &cobertura,
            r#"<?xml version="1.0"?><coverage branch-rate="0.5"></coverage>"#,
        )
        .unwrap();
        assert_eq!(
            CoverageFormat::detect(&cobertura).unwrap(),
            CoverageFormat::Cobertura
        );

        let jacoco = temp_dir.path().join("jacoco.xml");
        fs::write(
            &jacoco,
            r#"<?xml version="1.0"?><report><package/></report>"#,
        )
        .unwrap();
        assert_eq!(
            CoverageFormat::detect(&jacoco).unwrap(),
            CoverageFormat::JaCoCo
        );
    }

    #[test]
    fn test_expand_patterns_and_validation_helpers() {
        let base = Path::new("/tmp/project");
        let direct = CoverageDiscovery::expand_direct_pattern(base, "coverage.xml");
        assert!(direct.iter().any(|p| p
            .display()
            .to_string()
            .contains("target/coverage/coverage.xml")));

        let globs = CoverageDiscovery::expand_glob_pattern(Path::new("/work"), "**/coverage.xml");
        assert!(
            globs
                .iter()
                .any(|pattern| pattern.contains("**/coverage.xml")),
            "expected recursive pattern expansion"
        );
    }

    #[test]
    fn test_discover_coverage_files_and_filters() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();
        let coverage_dir = root.join("coverage");
        fs::create_dir_all(&coverage_dir).unwrap();
        let lcov_path = coverage_dir.join("lcov.info");
        fs::write(&lcov_path, "TN:\nSF:src/lib.rs\nDA:1,1\nend_of_record\n").unwrap();

        // Ensure there is a timestamp difference
        sleep(Duration::from_millis(50));

        let json_path = coverage_dir.join("coverage.json");
        fs::write(&json_path, r#"{"path": "src/lib.rs"}"#).unwrap();

        let mut config = CoverageConfig::default();
        config.search_paths = vec!["coverage".to_string()];
        config.file_patterns = vec!["lcov.info".to_string(), "coverage.json".to_string()];
        config.max_age_days = 0;

        let discovered =
            CoverageDiscovery::discover_coverage_files(root, &config).expect("discover coverage");
        assert_eq!(discovered.len(), 2);
        assert_eq!(discovered[0].format, CoverageFormat::IstanbulJson);
        assert_eq!(discovered[1].format, CoverageFormat::Lcov);

        let most_recent = CoverageDiscovery::get_most_recent(&discovered).unwrap();
        assert_eq!(most_recent.format, CoverageFormat::IstanbulJson);

        let lcov_only = CoverageDiscovery::filter_by_format(&discovered, CoverageFormat::Lcov);
        assert_eq!(lcov_only.len(), 1);
        assert_eq!(lcov_only[0].path, lcov_path);
    }
}
