//! Stack profiler integration for live reachability analysis.
//!
//! Provides ingestion and normalization of collapsed stack traces so they can be
//! merged into the call graph used by the live reachability pipeline.

use crate::core::errors::Result;
use glob::glob;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Language for symbol normalization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    Auto,
    Jvm,
    Py,
    Go,
    Node,
    Native,
}

impl Language {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Language::Auto),
            "jvm" | "java" => Ok(Language::Jvm),
            "py" | "python" => Ok(Language::Py),
            "go" => Ok(Language::Go),
            "node" | "js" | "javascript" => Ok(Language::Node),
            "native" | "c" | "cpp" | "rust" => Ok(Language::Native),
            _ => Err(crate::core::errors::ValknutError::validation(format!(
                "Unknown language: {}",
                s
            ))),
        }
    }
}

/// Timestamp source for edge data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimestampSource {
    FileMtime,
    Now,
    Rfc3339(String),
}

impl TimestampSource {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "filemtime" => Ok(TimestampSource::FileMtime),
            "now" => Ok(TimestampSource::Now),
            _ => Ok(TimestampSource::Rfc3339(s.to_string())),
        }
    }
}

/// Stack processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackConfig {
    pub svc: String,
    pub ver: String,
    pub lang: Language,
    pub ns_allow: Vec<String>,
    pub from: String,
    pub out: PathBuf,
    pub upload: Option<String>,
    pub fail_if_empty: bool,
    pub dry_run: bool,
    pub ts_source: TimestampSource,
    pub strip_prefix: Option<String>,
    pub dedupe: bool,
}

/// Stack processing result
#[derive(Debug, Clone)]
pub struct StackProcessingResult {
    pub files_processed: usize,
    pub samples_processed: u64,
    pub edges_before_filter: usize,
    pub edges_after_filter: usize,
    pub aggregated_edges: Vec<String>,
    pub warnings: Vec<String>,
}

/// Internal result for single file processing
#[derive(Debug, Clone)]
struct FileProcessingResult {
    pub samples: u64,
    pub edges_before: usize,
    pub edges: Vec<String>,
}

/// Stack processor (placeholder implementation)
pub struct StackProcessor {
    config: StackConfig,
}

impl StackProcessor {
    pub fn new(config: StackConfig) -> Result<Self> {
        Ok(Self { config })
    }

    pub async fn process(&self) -> Result<StackProcessingResult> {
        let mut warnings = Vec::new();
        let mut files_processed = 0;
        let mut samples_processed = 0;
        let mut edges_before_filter = 0;

        let mut aggregated_edges = Vec::new();

        let input_files = self.discover_input_files(&mut warnings)?;

        if input_files.is_empty() {
            return Err(crate::core::errors::ValknutError::validation(format!(
                "No stack files matched input pattern: {}",
                self.config.from
            )));
        }

        for path in input_files {
            match self.process_single_file(&path).await {
                Ok(result) => {
                    files_processed += 1;
                    samples_processed += result.samples;
                    edges_before_filter += result.edges_before;
                    aggregated_edges.extend(result.edges);
                }
                Err(err) => {
                    warnings.push(format!("Failed to process {}: {}", path.display(), err));
                }
            }
        }

        // Apply namespace filtering
        if !self.config.ns_allow.is_empty() {
            let original_count = aggregated_edges.len();
            aggregated_edges.retain(|edge| {
                self.config
                    .ns_allow
                    .iter()
                    .any(|prefix| edge.contains(prefix))
            });

            let filtered_count = aggregated_edges.len();
            if filtered_count < original_count {
                warnings.push(format!(
                    "Filtered {} edges due to namespace restrictions",
                    original_count - filtered_count
                ));
            }
        }

        // Deduplicate if requested
        if self.config.dedupe {
            let original_count = aggregated_edges.len();
            let mut seen = std::collections::BTreeSet::new();
            aggregated_edges.retain(|edge| seen.insert(edge.clone()));
            if aggregated_edges.len() < original_count {
                warnings.push(format!(
                    "Removed {} duplicate edges during deduplication",
                    original_count - aggregated_edges.len()
                ));
            }
        }

        aggregated_edges.sort();
        let edges_after_filter = aggregated_edges.len();

        if !self.config.dry_run {
            self.write_output(&aggregated_edges).await?;
        }

        if self.config.fail_if_empty && aggregated_edges.is_empty() {
            return Err(crate::core::errors::ValknutError::validation(
                "No edges were generated and fail_if_empty is set",
            ));
        }

        Ok(StackProcessingResult {
            files_processed,
            samples_processed,
            edges_before_filter,
            edges_after_filter,
            aggregated_edges,
            warnings,
        })
    }

    fn discover_input_files(&self, warnings: &mut Vec<String>) -> Result<Vec<PathBuf>> {
        let pattern = &self.config.from;
        let has_pattern = Self::contains_glob_char(pattern);
        let mut files = Vec::new();

        if has_pattern {
            match glob(pattern) {
                Ok(paths) => {
                    for entry in paths {
                        match entry {
                            Ok(path) if path.is_file() => files.push(path),
                            Ok(path) if path.is_dir() => {
                                files.extend(self.collect_stack_files_from_dir(&path));
                            }
                            Ok(_) => {}
                            Err(err) => warnings.push(format!(
                                "Failed to read path for pattern {}: {}",
                                pattern, err
                            )),
                        }
                    }
                }
                Err(err) => {
                    return Err(crate::core::errors::ValknutError::validation(format!(
                        "Invalid glob pattern '{}': {}",
                        pattern, err
                    )));
                }
            }
        } else {
            let path = PathBuf::from(pattern);
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                files.extend(self.collect_stack_files_from_dir(&path));
            } else {
                return Err(crate::core::errors::ValknutError::validation(format!(
                    "Source path does not exist: {}",
                    pattern
                )));
            }
        }

        let mut files = files;
        files.sort();
        Ok(files)
    }

    fn collect_stack_files_from_dir(&self, dir: &Path) -> Vec<PathBuf> {
        WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.into_path())
            .collect()
    }

    async fn process_single_file(&self, file_path: &Path) -> Result<FileProcessingResult> {
        let content = tokio::fs::read_to_string(file_path).await?;
        let language = self.determine_language(file_path, &content);
        let edges = self.extract_edges(&content, &language);

        let normalized_edges: Vec<String> = edges
            .into_iter()
            .map(|edge| self.apply_strip_prefix(edge))
            .collect();

        Ok(FileProcessingResult {
            samples: normalized_edges.len() as u64,
            edges_before: normalized_edges.len(),
            edges: normalized_edges,
        })
    }

    fn determine_language(&self, file_path: &Path, content: &str) -> Language {
        match self.config.lang {
            Language::Auto => self.detect_language(file_path, content),
            ref lang => lang.clone(),
        }
    }

    fn detect_language(&self, file_path: &Path, content: &str) -> Language {
        if let Some(extension) = file_path.extension().and_then(|ext| ext.to_str()) {
            match extension {
                "py" => return Language::Py,
                "go" => return Language::Go,
                "js" | "cjs" | "mjs" => return Language::Node,
                "ts" | "tsx" => return Language::Node,
                "rs" | "cpp" | "cc" | "c" | "h" | "hpp" => return Language::Native,
                "java" | "kt" => return Language::Jvm,
                _ => {}
            }
        }

        let trimmed = content.trim_start();
        if trimmed.contains("File \"") && trimmed.contains("line ") {
            Language::Py
        } else if trimmed.contains(".go:") {
            Language::Go
        } else if trimmed.contains(" at ") && trimmed.contains(".js") {
            Language::Node
        } else if trimmed.contains(" at ") && trimmed.contains("::") {
            Language::Native
        } else if trimmed.contains(" at ") && trimmed.contains('(') {
            Language::Jvm
        } else {
            Language::Native
        }
    }

    fn extract_edges(&self, content: &str, language: &Language) -> Vec<String> {
        let mut edges = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            let edge = match language {
                Language::Jvm => self.extract_jvm_edge(trimmed),
                Language::Py => self.extract_python_edge(trimmed),
                Language::Go => self.extract_go_edge(trimmed),
                Language::Node => self.extract_node_edge(trimmed),
                Language::Native => self.extract_native_edge(trimmed),
                Language::Auto => None,
            };

            if let Some(edge) = edge {
                edges.push(edge);
            }
        }

        edges
    }

    fn apply_strip_prefix(&self, edge: String) -> String {
        if let Some(prefix) = &self.config.strip_prefix {
            edge.strip_prefix(prefix).unwrap_or(&edge).to_string()
        } else {
            edge
        }
    }

    async fn write_output(&self, edges: &[String]) -> Result<()> {
        if self.config.dry_run {
            return Ok(());
        }

        if let Some(parent) = self.config.out.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let output_content = edges.join("\n");
        tokio::fs::write(&self.config.out, output_content).await?;
        Ok(())
    }

    fn extract_jvm_edge(&self, line: &str) -> Option<String> {
        // Extract JVM stack trace edge: "at com.example.Class.method(Class.java:123)"
        if let Some(start) = line.find("at ") {
            let method_part = &line[start + 3..];
            if let Some(paren_pos) = method_part.find('(') {
                let full_method = &method_part[..paren_pos];
                if let Some(dot_pos) = full_method.rfind('.') {
                    let class_name = &full_method[..dot_pos];
                    let method_name = &full_method[dot_pos + 1..];
                    return Some(format!("{}::{}", class_name, method_name));
                }
            }
        }
        None
    }

    fn extract_python_edge(&self, line: &str) -> Option<String> {
        // Extract Python stack trace edge
        if line.trim().starts_with("File \"") {
            // File line: File "/path/to/file.py", line 123, in function_name
            if let Some(in_pos) = line.find(" in ") {
                let function_name = line[in_pos + 4..].trim();
                if let Some(file_start) = line.find("File \"") {
                    if let Some(file_end) = line[file_start + 6..].find('"') {
                        let file_path = &line[file_start + 6..file_start + 6 + file_end];
                        let file_name = std::path::Path::new(file_path)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown");
                        return Some(format!("{}::{}", file_name, function_name));
                    }
                }
            }
        }
        None
    }

    fn extract_go_edge(&self, line: &str) -> Option<String> {
        // Extract Go stack trace edge: "package.function(file.go:123)"
        if let Some(go_pos) = line.find(".go:") {
            let before_go = &line[..go_pos];
            if let Some(paren_pos) = before_go.rfind('(') {
                let function_part = &before_go[..paren_pos];
                if let Some(space_pos) = function_part.rfind(' ') {
                    return Some(function_part[space_pos + 1..].to_string());
                } else {
                    return Some(function_part.to_string());
                }
            }
        }
        None
    }

    fn extract_node_edge(&self, line: &str) -> Option<String> {
        // Extract Node.js stack trace edge: "at Object.function (/path/file.js:123:45)"
        if line.trim().starts_with("at ") {
            let method_part = line.trim()[3..].trim();
            if let Some(paren_pos) = method_part.find('(') {
                let function_name = &method_part[..paren_pos].trim();
                return Some(function_name.to_string());
            }
        }
        None
    }

    fn extract_native_edge(&self, line: &str) -> Option<String> {
        // Extract native stack trace edge (C++/Rust style)
        if line.contains("::") {
            // Look for function names with namespace separators
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in parts {
                if part.contains("::") && !part.starts_with('/') {
                    return Some(part.to_string());
                }
            }
        }
        None
    }

    fn contains_glob_char(pattern: &str) -> bool {
        pattern.chars().any(|ch| matches!(ch, '*' | '?' | '['))
    }
}
