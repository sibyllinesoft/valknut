//! File utilities for safe and robust file operations.
//!
//! This module provides utilities for reading files with proper UTF-8 handling,
//! binary file detection, and encoding conversion capabilities.

use std::path::Path;
use std::fs;
use tracing::warn;
use crate::core::errors::{ValknutError, Result};

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
            .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
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