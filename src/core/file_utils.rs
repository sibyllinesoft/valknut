//! File utilities for safe and robust file operations.
//!
//! This module provides utilities for reading files with proper UTF-8 handling,
//! binary file detection, encoding conversion capabilities, and coverage file discovery.

use std::path::{Path, PathBuf};
use std::fs;
use std::time::{Duration, SystemTime};
use tracing::{warn, info, debug};
use crate::core::errors::{ValknutError, Result};
use crate::core::config::CoverageConfig;

/// Safe file reading with UTF-8 validation and fallback handling
pub struct FileReader;

impl FileReader {
    /// Read a file to string, handling non-UTF-8 files gracefully
    pub fn read_to_string(file_path: &Path) -> Result<String> {
        // First, check if the file is likely to be binary
        if Self::is_likely_binary(file_path)? {
            return Err(ValknutError::validation(
                format!("File appears to be binary: {}", file_path.display())
            ));
        }

        // Try to read as UTF-8 first
        match fs::read_to_string(file_path) {
            Ok(content) => Ok(content),
            Err(e) => {
                // Check if this is a UTF-8 error by looking at the error kind
                if e.kind() == std::io::ErrorKind::InvalidData {
                    // Try to read as bytes and convert with lossy UTF-8
                    let bytes = fs::read(file_path)
                        .map_err(|err| ValknutError::io("Failed to read file as bytes", err))?;
                    
                    let content = String::from_utf8_lossy(&bytes).to_string();
                    warn!("File contained invalid UTF-8, converted with lossy encoding: {}", file_path.display());
                    Ok(content)
                } else {
                    Err(ValknutError::io("Failed to read file", e))
                }
            }
        }
    }

    /// Check if a file is likely to be binary based on extension and content sampling
    pub fn is_likely_binary(file_path: &Path) -> Result<bool> {
        // Check extension first
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            let binary_extensions = [
                // Archives
                "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
                // Images
                "png", "jpg", "jpeg", "gif", "bmp", "svg", "ico", "webp",
                // Audio/Video
                "mp3", "mp4", "avi", "wav", "flv", "mov", "wmv", "mkv",
                // Documents
                "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
                // Executables
                "exe", "dll", "so", "dylib", "bin", "deb", "rpm",
                // Others
                "sqlite", "db", "woff", "woff2", "ttf", "eot",
            ];
            
            if binary_extensions.iter().any(|&ext| extension.eq_ignore_ascii_case(ext)) {
                return Ok(true);
            }
        }

        // For files without clear extensions, sample the first few bytes
        let metadata = fs::metadata(file_path)
            .map_err(|e| ValknutError::io("Failed to read file metadata", e))?;
        
        // Don't process very large files
        if metadata.len() > 10 * 1024 * 1024 { // 10MB limit
            return Ok(true);
        }

        // Sample first 1024 bytes to check for binary content
        let sample_size = std::cmp::min(1024, metadata.len() as usize);
        let mut buffer = vec![0u8; sample_size];
        
        use std::io::Read;
        let mut file = fs::File::open(file_path)
            .map_err(|e| ValknutError::io("Failed to open file for sampling", e))?;
        
        file.read_exact(&mut buffer)
            .map_err(|e| ValknutError::io("Failed to read file sample", e))?;

        // Check for null bytes (common indicator of binary content)
        let null_bytes = buffer.iter().filter(|&&b| b == 0).count();
        let null_percentage = (null_bytes as f64 / buffer.len() as f64) * 100.0;
        
        // If more than 1% null bytes, likely binary
        Ok(null_percentage > 1.0)
    }

    /// Count lines of code in a file, skipping binary files and handling encoding issues
    pub fn count_lines_of_code(file_path: &Path) -> Result<usize> {
        if Self::is_likely_binary(file_path)? {
            return Ok(0); // Binary files have no lines of code
        }

        let content = Self::read_to_string(file_path)?;
        Ok(content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() 
                    && !trimmed.starts_with("//") 
                    && !trimmed.starts_with("#")
            })
            .count())
    }

    /// Check if a file has a supported programming language extension
    pub fn is_code_file(file_path: &Path) -> bool {
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            matches!(
                extension.to_lowercase().as_str(),
                "py" | "js" | "ts" | "jsx" | "tsx" | "rs" | "go" | "java" 
                | "cpp" | "c" | "h" | "hpp" | "cs" | "php" | "rb" | "kt" 
                | "swift" | "scala" | "clj" | "hs" | "ml" | "fs" | "elm"
                | "dart" | "lua" | "perl" | "r" | "jl" | "nim" | "zig"
            )
        } else {
            false
        }
    }
}

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
    CoveragePyXml,      // coverage.py XML format
    Lcov,               // LCOV .info format
    Cobertura,          // Cobertura XML format
    JaCoCo,             // JaCoCo XML format
    IstanbulJson,       // Istanbul JSON format
    Unknown,
}

impl CoverageFormat {
    /// Detect format from file path and content
    pub fn detect(file_path: &Path) -> Result<Self> {
        let filename = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        // First try to detect by filename
        if filename.contains("coverage") && filename.ends_with(".xml") {
            return Ok(Self::CoveragePyXml);
        }
        
        if filename.ends_with("lcov.info") || filename == "lcov.info" || filename.ends_with(".lcov") {
            return Ok(Self::Lcov);
        }
        
        if filename.contains("cobertura") && filename.ends_with(".xml") {
            return Ok(Self::Cobertura);
        }
        
        if filename.ends_with(".json") {
            return Ok(Self::IstanbulJson);
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
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| ValknutError::io("Failed to read coverage file for format detection", e))?;
        
        let first_kb = content.chars().take(1024).collect::<String>().to_lowercase();
        
        if first_kb.contains("<?xml") {
            if first_kb.contains("coverage") && first_kb.contains("branch-rate") {
                Ok(Self::Cobertura)
            } else if first_kb.contains("coverage") {
                Ok(Self::CoveragePyXml)
            } else if first_kb.contains("report") && first_kb.contains("package") {
                Ok(Self::JaCoCo)
            } else {
                Ok(Self::Unknown)
            }
        } else if first_kb.starts_with("tn:") || first_kb.contains("\ntn:") || first_kb.starts_with("sf:") || first_kb.contains("\nsf:") {
            Ok(Self::Lcov)
        } else if first_kb.starts_with("{") && first_kb.contains("\"path\"") {
            Ok(Self::IstanbulJson)
        } else {
            Ok(Self::Unknown)
        }
    }
}

/// Coverage file discovery utility
pub struct CoverageDiscovery;

impl CoverageDiscovery {
    /// Discover coverage files in the given root path using configuration
    pub fn discover_coverage_files(
        root_path: &Path,
        config: &CoverageConfig,
    ) -> Result<Vec<CoverageFile>> {
        debug!("Coverage discovery called with root_path: {}, coverage_file: {:?}, auto_discover: {}", 
               root_path.display(), config.coverage_file, config.auto_discover);
        
        if let Some(ref explicit_file) = config.coverage_file {
            debug!("Using explicit coverage file: {}", explicit_file.display());
            // Use explicitly specified coverage file
            return Self::validate_coverage_file(explicit_file);
        }
        
        if !config.auto_discover {
            return Ok(Vec::new());
        }
        
        debug!("Starting coverage file discovery in: {}", root_path.display());
        
        let mut discovered_files = Vec::new();
        let max_age = if config.max_age_days > 0 {
            Some(Duration::from_secs(config.max_age_days as u64 * 24 * 60 * 60))
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
            info!("  Found: {} (format: {:?}, size: {} bytes)", 
                  file.path.display(), file.format, file.size);
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
        
        // Handle glob patterns
        if pattern.contains("*") {
            // Use glob matching with multiple strategies
            let glob_patterns = Self::expand_glob_pattern(search_path, pattern);
            
            for glob_pattern in glob_patterns {
                match glob::glob(&glob_pattern) {
                    Ok(paths) => {
                        for entry in paths {
                            if let Ok(path) = entry {
                                if let Ok(coverage_file) = Self::validate_coverage_file_with_age(&path, max_age) {
                                    if let Some(file) = coverage_file {
                                        files.push(file);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Glob pattern failed: {}: {}", glob_pattern, e);
                    }
                }
            }
        } else {
            // Direct file lookup with intelligent fallbacks
            let candidate_paths = Self::expand_direct_pattern(search_path, pattern);
            
            for file_path in candidate_paths {
                if let Ok(coverage_file) = Self::validate_coverage_file_with_age(&file_path, max_age) {
                    if let Some(file) = coverage_file {
                        files.push(file);
                    }
                }
            }
        }
        
        Ok(files)
    }
    
    /// Expand glob pattern into multiple search strategies
    fn expand_glob_pattern(search_path: &Path, pattern: &str) -> Vec<String> {
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
    
    /// Expand direct pattern into intelligent fallback paths
    fn expand_direct_pattern(search_path: &Path, pattern: &str) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        
        // Primary path
        paths.push(search_path.join(pattern));
        
        // Common variations for coverage files
        if pattern == "coverage.xml" {
            paths.push(search_path.join("coverage/coverage.xml"));
            paths.push(search_path.join("target/coverage/coverage.xml"));
            paths.push(search_path.join("target/tarpaulin/coverage.xml"));
            paths.push(search_path.join("test-results/coverage.xml"));
            paths.push(search_path.join("reports/coverage.xml"));
        } else if pattern == "lcov.info" {
            paths.push(search_path.join("coverage/lcov.info"));
            paths.push(search_path.join("coverage-reports/lcov.info"));
            paths.push(search_path.join("target/coverage/lcov.info"));
        } else if pattern == "coverage.json" {
            paths.push(search_path.join("coverage/coverage-final.json"));
            paths.push(search_path.join("coverage/coverage.json"));
            paths.push(search_path.join("reports/coverage.json"));
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
        
        let modified = metadata.modified()
            .map_err(|e| ValknutError::io("Failed to get file modification time", e))?;
        
        // Check age if specified
        if let Some(max_age) = max_age {
            if let Ok(elapsed) = modified.elapsed() {
                if elapsed > max_age {
                    debug!("Coverage file too old: {} (age: {:?})", file_path.display(), elapsed);
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
    use tempfile::TempDir;

    #[test]
    fn test_read_valid_utf8() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello, world! 🦀").unwrap();

        let content = FileReader::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, world! 🦀");
    }

    #[test]
    fn test_binary_detection_by_extension() {
        let temp_dir = TempDir::new().unwrap();
        let binary_file = temp_dir.path().join("test.png");
        fs::write(&binary_file, b"\x89PNG\r\n\x1a\n").unwrap();

        assert!(FileReader::is_likely_binary(&binary_file).unwrap());
    }

    #[test]
    fn test_code_file_detection() {
        assert!(FileReader::is_code_file(Path::new("test.rs")));
        assert!(FileReader::is_code_file(Path::new("test.py")));
        assert!(FileReader::is_code_file(Path::new("test.js")));
        assert!(!FileReader::is_code_file(Path::new("test.png")));
        assert!(!FileReader::is_code_file(Path::new("test.txt")));
    }

    #[test]
    fn test_count_lines_of_code() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(&file_path, "# Comment\ndef hello():\n    print('hello')\n\n").unwrap();

        let loc = FileReader::count_lines_of_code(&file_path).unwrap();
        assert_eq!(loc, 2); // Only non-empty, non-comment lines
    }
}