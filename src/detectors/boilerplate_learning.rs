//! Self-Learning Boilerplate Detection System
//!
//! This module implements an adaptive boilerplate detection system that:
//! - Mines frequent shingles and PDG motifs across the codebase
//! - Identifies top-K frequent patterns as "stop motifs"
//! - Down-weights stop motifs during similarity computation
//! - Implements weekly cache refresh for adaptation
//! - Provides hub suppressor for logging/metrics/HTTP patterns

use std::collections::{HashMap, HashSet, BTreeMap};
use std::sync::{Arc, RwLock};
use std::path::Path;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
// Removed regex import - using tree-sitter exclusively

use crate::core::errors::{Result, ValknutError};
use crate::detectors::clone_detection::{PdgMotif, TfIdfAnalyzer};
use crate::lang::{LanguageAdapter, python::PythonAdapter, javascript::JavaScriptAdapter, typescript::TypeScriptAdapter, go::GoAdapter, rust_lang::RustAdapter};

/// Self-learning boilerplate detection system
#[derive(Debug)]
pub struct BoilerplateLearningSystem {
    /// Frequent pattern miner
    pattern_miner: FrequentPatternMiner,
    
    /// Stop motif database
    stop_motifs: Arc<RwLock<StopMotifDatabase>>,
    
    /// Hub suppressor for common patterns
    hub_suppressor: HubSuppressor,
    
    /// Cache manager for persistence
    cache_manager: BoilerplateCacheManager,
    
    /// Learning configuration
    config: BoilerplateLearningConfig,
    
    /// Last refresh timestamp
    last_refresh: DateTime<Utc>,
}

impl BoilerplateLearningSystem {
    /// Create a new boilerplate learning system
    pub fn new(config: BoilerplateLearningConfig) -> Self {
        Self {
            pattern_miner: FrequentPatternMiner::new(config.min_support_threshold),
            stop_motifs: Arc::new(RwLock::new(StopMotifDatabase::new())),
            hub_suppressor: HubSuppressor::new(),
            cache_manager: BoilerplateCacheManager::new(),
            config,
            last_refresh: Utc::now(),
        }
    }
    
    /// Learn boilerplate patterns from a codebase
    pub async fn learn_from_codebase(&mut self, codebase_path: &Path) -> Result<LearningReport> {
        let start_time = Utc::now();
        let mut report = LearningReport::new();
        
        // Mine frequent shingles
        let shingle_patterns = self.mine_frequent_shingles(codebase_path).await?;
        report.shingles_analyzed = shingle_patterns.len();
        
        // Mine frequent PDG motifs
        let motif_patterns = self.mine_frequent_motifs(codebase_path).await?;
        report.motifs_analyzed = motif_patterns.len();
        
        // Identify stop motifs (top percentile by frequency)
        let stop_shingles = self.identify_stop_shingles(&shingle_patterns);
        let stop_motifs = self.identify_stop_motifs(&motif_patterns);
        
        // Update stop motif database
        {
            let mut db = self.stop_motifs.write().unwrap();
            db.update_stop_shingles(stop_shingles);
            db.update_stop_motifs(stop_motifs);
            db.last_updated = Utc::now();
        }
        
        // Update hub suppressor patterns
        self.hub_suppressor.update_hub_patterns(&shingle_patterns, &motif_patterns).await?;
        
        // Cache results
        self.cache_manager.save_cache(&self.stop_motifs, &self.hub_suppressor).await?;
        
        report.stop_shingles_identified = {
            let db = self.stop_motifs.read().unwrap();
            db.stop_shingles.len()
        };
        report.stop_motifs_identified = {
            let db = self.stop_motifs.read().unwrap();
            db.stop_motifs.len()
        };
        report.learning_duration = Utc::now().signed_duration_since(start_time);
        
        Ok(report)
    }
    
    /// Check if automatic refresh is needed
    pub fn needs_refresh(&self) -> bool {
        let refresh_interval = Duration::days(self.config.refresh_interval_days);
        Utc::now().signed_duration_since(self.last_refresh) >= refresh_interval
    }
    
    /// Get weight for a shingle (down-weighted if it's a stop motif)
    pub fn get_shingle_weight(&self, shingle: &str) -> f64 {
        let db = self.stop_motifs.read().unwrap();
        
        if let Some(frequency) = db.stop_shingles.get(shingle) {
            // Down-weight by configured factor
            1.0 - (self.config.stop_motif_downweight * (*frequency as f64 / db.max_shingle_frequency as f64))
        } else {
            1.0 // Full weight for non-stop motifs
        }
    }
    
    /// Get weight for a PDG motif
    pub fn get_motif_weight(&self, motif: &PdgMotif) -> f64 {
        let db = self.stop_motifs.read().unwrap();
        
        if let Some(frequency) = db.stop_motifs.get(&motif.wl_hash) {
            1.0 - (self.config.stop_motif_downweight * (*frequency as f64 / db.max_motif_frequency as f64))
        } else {
            1.0
        }
    }
    
    /// Check if a pattern should be suppressed as hub boilerplate
    pub fn is_hub_pattern(&self, pattern: &str) -> bool {
        self.hub_suppressor.is_hub_pattern(pattern)
    }
    
    /// Mine frequent shingles from codebase
    async fn mine_frequent_shingles(&mut self, codebase_path: &Path) -> Result<HashMap<String, usize>> {
        self.pattern_miner.mine_shingles(codebase_path).await
    }
    
    /// Mine frequent PDG motifs from codebase
    async fn mine_frequent_motifs(&mut self, codebase_path: &Path) -> Result<HashMap<String, usize>> {
        self.pattern_miner.mine_motifs(codebase_path).await
    }
    
    /// Identify top percentile shingles as stop motifs
    fn identify_stop_shingles(&self, patterns: &HashMap<String, usize>) -> HashMap<String, usize> {
        let mut sorted_patterns: Vec<_> = patterns.iter().collect();
        sorted_patterns.sort_by(|a, b| b.1.cmp(a.1));
        
        let cutoff = (sorted_patterns.len() as f64 * self.config.stop_motif_percentile / 100.0).ceil() as usize;
        
        sorted_patterns
            .into_iter()
            .take(cutoff)
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }
    
    /// Identify top percentile motifs as stop motifs
    fn identify_stop_motifs(&self, patterns: &HashMap<String, usize>) -> HashMap<String, usize> {
        self.identify_stop_shingles(patterns) // Same logic
    }
}

/// Frequent pattern miner for shingles and motifs
#[derive(Debug)]
pub struct FrequentPatternMiner {
    min_support: usize,
    shingle_size: usize,
}

impl FrequentPatternMiner {
    fn new(min_support: usize) -> Self {
        Self {
            min_support,
            shingle_size: 8, // k=8 as specified
        }
    }
    
    /// Mine frequent shingles from codebase
    async fn mine_shingles(&self, codebase_path: &Path) -> Result<HashMap<String, usize>> {
        let mut shingle_counts = HashMap::new();
        
        // Walk through all source files
        let walker = walkdir::WalkDir::new(codebase_path);
        
        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if self.is_source_file(entry.path()) {
                let content = tokio::fs::read_to_string(entry.path()).await
                    .map_err(|e| ValknutError::io(format!("Failed to read file: {:?}", entry.path()), e))?;
                
                let shingles = self.extract_shingles(&content);
                
                for shingle in shingles {
                    *shingle_counts.entry(shingle).or_insert(0) += 1;
                }
            }
        }
        
        // Filter by minimum support
        let frequent_shingles: HashMap<String, usize> = shingle_counts
            .into_iter()
            .filter(|(_, count)| *count >= self.min_support)
            .collect();
        
        Ok(frequent_shingles)
    }
    
    /// Mine frequent motifs from codebase
    async fn mine_motifs(&self, codebase_path: &Path) -> Result<HashMap<String, usize>> {
        let mut motif_counts = HashMap::new();
        
        let walker = walkdir::WalkDir::new(codebase_path);
        
        for entry in walker.into_iter().filter_map(|e| e.ok()) {
            if self.is_source_file(entry.path()) {
                let content = tokio::fs::read_to_string(entry.path()).await
                    .map_err(|e| ValknutError::io(format!("Failed to read file: {:?}", entry.path()), e))?;
                
                let motifs = self.extract_motifs(&content).await?;
                
                for motif in motifs {
                    *motif_counts.entry(motif.wl_hash).or_insert(0) += 1;
                }
            }
        }
        
        // Filter by minimum support
        let frequent_motifs: HashMap<String, usize> = motif_counts
            .into_iter()
            .filter(|(_, count)| *count >= self.min_support)
            .collect();
        
        Ok(frequent_motifs)
    }
    
    /// Check if a file is a source code file
    fn is_source_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            matches!(
                extension.to_str().unwrap_or(""),
                "rs" | "py" | "js" | "ts" | "java" | "cpp" | "c" | "h" | "hpp" | "go"
            )
        } else {
            false
        }
    }
    
    /// Extract k-shingles from source code
    fn extract_shingles(&self, content: &str) -> Vec<String> {
        let mut shingles = Vec::new();
        
        // Normalize and tokenize
        let tokens: Vec<&str> = content
            .lines()
            .flat_map(|line| {
                let line = line.trim();
                if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                    Vec::new()
                } else {
                    line.split_whitespace().collect()
                }
            })
            .collect();
        
        // Create k-shingles
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                let shingle = tokens[i..i + self.shingle_size].join(" ");
                shingles.push(shingle);
            }
        }
        
        shingles
    }
    
    /// Extract PDG motifs from source code
    async fn extract_motifs(&self, content: &str) -> Result<Vec<PdgMotif>> {
        // Use the PDG motif analyzer from clone_detection module
        let mut motif_analyzer = crate::detectors::clone_detection::PdgMotifAnalyzer::new(3);
        let motifs = motif_analyzer.extract_motifs(content, "temp_entity");
        Ok(motifs)
    }
}

/// Stop motif database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopMotifDatabase {
    /// Stop shingles with their frequencies
    pub stop_shingles: HashMap<String, usize>,
    
    /// Stop motifs with their frequencies
    pub stop_motifs: HashMap<String, usize>,
    
    /// Maximum shingle frequency for normalization
    pub max_shingle_frequency: usize,
    
    /// Maximum motif frequency for normalization
    pub max_motif_frequency: usize,
    
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl StopMotifDatabase {
    fn new() -> Self {
        Self {
            stop_shingles: HashMap::new(),
            stop_motifs: HashMap::new(),
            max_shingle_frequency: 1,
            max_motif_frequency: 1,
            last_updated: Utc::now(),
        }
    }
    
    /// Update stop shingles
    fn update_stop_shingles(&mut self, new_shingles: HashMap<String, usize>) {
        self.max_shingle_frequency = new_shingles.values().max().copied().unwrap_or(1);
        self.stop_shingles = new_shingles;
    }
    
    /// Update stop motifs
    fn update_stop_motifs(&mut self, new_motifs: HashMap<String, usize>) {
        self.max_motif_frequency = new_motifs.values().max().copied().unwrap_or(1);
        self.stop_motifs = new_motifs;
    }
}

/// Hub suppressor for common infrastructure patterns (tree-sitter based)
#[derive(Debug)]
pub struct HubSuppressor {
    /// Hub patterns (logging, metrics, HTTP routing, etc.)
    hub_patterns: HashSet<HubPattern>,
    
    /// Hub suppression threshold
    threshold: f64,
}

impl HubSuppressor {
    fn new() -> Self {
        let mut suppressor = Self {
            hub_patterns: HashSet::new(),
            threshold: 0.6,
        };
        
        suppressor.initialize_default_patterns();
        suppressor
    }
    
    /// Initialize default hub patterns (tree-sitter based)
    fn initialize_default_patterns(&mut self) {
        let default_patterns = vec![
            // Logging patterns
            HubPattern {
                pattern_type: HubPatternType::Logging,
                pattern: "log(".to_string(),
                description: "Logging calls".to_string(),
            },
            HubPattern {
                pattern_type: HubPatternType::Logging,
                pattern: "debug(".to_string(),
                description: "Debug logging".to_string(),
            },
            HubPattern {
                pattern_type: HubPatternType::Logging,
                pattern: "info(".to_string(),
                description: "Info logging".to_string(),
            },
            
            // Metrics patterns
            HubPattern {
                pattern_type: HubPatternType::Metrics,
                pattern: "counter.".to_string(),
                description: "Counter metrics".to_string(),
            },
            HubPattern {
                pattern_type: HubPatternType::Metrics,
                pattern: "histogram.".to_string(),
                description: "Histogram metrics".to_string(),
            },
            
            // HTTP router patterns
            HubPattern {
                pattern_type: HubPatternType::HttpRouter,
                pattern: ".get(".to_string(),
                description: "HTTP GET routes".to_string(),
            },
            HubPattern {
                pattern_type: HubPatternType::HttpRouter,
                pattern: ".post(".to_string(),
                description: "HTTP POST routes".to_string(),
            },
            
            // Database patterns
            HubPattern {
                pattern_type: HubPatternType::Database,
                pattern: "SELECT".to_string(),
                description: "SQL SELECT queries".to_string(),
            },
            HubPattern {
                pattern_type: HubPatternType::Database,
                pattern: "INSERT".to_string(),
                description: "SQL INSERT queries".to_string(),
            },
            
            // Test patterns
            HubPattern {
                pattern_type: HubPatternType::Testing,
                pattern: "test".to_string(),
                description: "Test functions".to_string(),
            },
            HubPattern {
                pattern_type: HubPatternType::Testing,
                pattern: "assert".to_string(),
                description: "Assert statements".to_string(),
            },
        ];
        
        for pattern in default_patterns {
            self.hub_patterns.insert(pattern);
        }
    }
    
    /// Update hub patterns based on frequent patterns
    async fn update_hub_patterns(
        &mut self, 
        shingles: &HashMap<String, usize>, 
        motifs: &HashMap<String, usize>
    ) -> Result<()> {
        // Analyze shingles for hub patterns
        for (shingle, frequency) in shingles {
            if *frequency as f64 > (shingles.len() as f64 * self.threshold) {
                self.add_hub_pattern_from_shingle(shingle).await?;
            }
        }
        
        // Analyze motifs for hub patterns
        for (motif_hash, frequency) in motifs {
            if *frequency as f64 > (motifs.len() as f64 * self.threshold) {
                self.add_hub_pattern_from_motif(motif_hash).await?;
            }
        }
        
        Ok(())
    }
    
    /// Add hub pattern from frequent shingle (tree-sitter based)
    async fn add_hub_pattern_from_shingle(&mut self, shingle: &str) -> Result<()> {
        // Analyze shingle to determine pattern type
        let pattern_type = self.classify_shingle_pattern(shingle);
        
        if pattern_type != HubPatternType::Unknown {
            let pattern = HubPattern {
                pattern_type,
                pattern: shingle.to_string(),
                description: format!("Learned hub pattern: {}", shingle),
            };
            
            self.hub_patterns.insert(pattern);
        }
        
        Ok(())
    }
    
    /// Add hub pattern from frequent motif
    async fn add_hub_pattern_from_motif(&mut self, _motif_hash: &str) -> Result<()> {
        // Motif-based hub pattern detection would be implemented here
        // For now, we focus on shingle-based patterns
        Ok(())
    }
    
    /// Classify a shingle to determine if it's a hub pattern
    fn classify_shingle_pattern(&self, shingle: &str) -> HubPatternType {
        let lower = shingle.to_lowercase();
        
        if lower.contains("log") || lower.contains("debug") || lower.contains("info") || 
           lower.contains("warn") || lower.contains("error") {
            HubPatternType::Logging
        } else if lower.contains("metric") || lower.contains("counter") || lower.contains("gauge") {
            HubPatternType::Metrics
        } else if lower.contains("get(") || lower.contains("post(") || lower.contains("route") {
            HubPatternType::HttpRouter
        } else if lower.contains("select") || lower.contains("insert") || lower.contains("query") {
            HubPatternType::Database
        } else if lower.contains("test") || lower.contains("assert") || lower.contains("expect") {
            HubPatternType::Testing
        } else {
            HubPatternType::Unknown
        }
    }
    
    /// Check if a pattern should be suppressed as hub boilerplate using tree-sitter
    pub fn is_hub_pattern(&self, source_code: &str, file_path: &str) -> bool {
        // Use tree-sitter to analyze the source code for hub patterns
        let language = self.detect_language_from_path(file_path);
        
        match language.as_str() {
            "python" => {
                if let Ok(mut adapter) = PythonAdapter::new() {
                    let patterns: Vec<String> = self.hub_patterns.iter().map(|p| p.pattern.clone()).collect();
                    if let Ok(found_patterns) = adapter.contains_boilerplate_patterns(source_code, &patterns) {
                        return !found_patterns.is_empty();
                    }
                }
            }
            "javascript" => {
                if let Ok(mut adapter) = JavaScriptAdapter::new() {
                    let patterns: Vec<String> = self.hub_patterns.iter().map(|p| p.pattern.clone()).collect();
                    if let Ok(found_patterns) = adapter.contains_boilerplate_patterns(source_code, &patterns) {
                        return !found_patterns.is_empty();
                    }
                }
            }
            "typescript" => {
                if let Ok(mut adapter) = TypeScriptAdapter::new() {
                    let patterns: Vec<String> = self.hub_patterns.iter().map(|p| p.pattern.clone()).collect();
                    if let Ok(found_patterns) = adapter.contains_boilerplate_patterns(source_code, &patterns) {
                        return !found_patterns.is_empty();
                    }
                }
            }
            "go" => {
                if let Ok(mut adapter) = GoAdapter::new() {
                    let patterns: Vec<String> = self.hub_patterns.iter().map(|p| p.pattern.clone()).collect();
                    if let Ok(found_patterns) = adapter.contains_boilerplate_patterns(source_code, &patterns) {
                        return !found_patterns.is_empty();
                    }
                }
            }
            "rust" => {
                if let Ok(mut adapter) = RustAdapter::new() {
                    let patterns: Vec<String> = self.hub_patterns.iter().map(|p| p.pattern.clone()).collect();
                    if let Ok(found_patterns) = adapter.contains_boilerplate_patterns(source_code, &patterns) {
                        return !found_patterns.is_empty();
                    }
                }
            }
            _ => {}
        }
        
        false // If tree-sitter parsing fails, assume not a hub pattern
    }
    
    /// Detect programming language from file path
    fn detect_language_from_path(&self, file_path: &str) -> String {
        let path = std::path::Path::new(file_path);
        if let Some(extension) = path.extension() {
            match extension.to_str().unwrap_or("") {
                "py" => "python".to_string(),
                "js" => "javascript".to_string(),
                "ts" | "tsx" => "typescript".to_string(),
                "go" => "go".to_string(),
                "rs" => "rust".to_string(),
                _ => "unknown".to_string(),
            }
        } else {
            "unknown".to_string()
        }
    }
}

/// Hub pattern for common infrastructure code (tree-sitter based)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HubPattern {
    pub pattern_type: HubPatternType,
    pub pattern: String,
    pub description: String,
}

/// Type of hub pattern
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HubPatternType {
    Logging,
    Metrics,
    HttpRouter,
    Database,
    Testing,
    Unknown,
}

/// Boilerplate cache manager for persistence
#[derive(Debug)]
pub struct BoilerplateCacheManager {
    cache_dir: std::path::PathBuf,
}

impl BoilerplateCacheManager {
    fn new() -> Self {
        Self {
            cache_dir: std::path::PathBuf::from(".valknut/cache"),
        }
    }
    
    /// Save cache to disk
    async fn save_cache(
        &self,
        stop_motifs: &Arc<RwLock<StopMotifDatabase>>,
        hub_suppressor: &HubSuppressor,
    ) -> Result<()> {
        // Ensure cache directory exists
        tokio::fs::create_dir_all(&self.cache_dir).await
            .map_err(|e| ValknutError::io("Failed to create cache directory".to_string(), e))?;
        
        // Save stop motifs
        {
            let db = stop_motifs.read().unwrap();
            let serialized = serde_json::to_string_pretty(&*db)?;
            let stop_motifs_path = self.cache_dir.join("stop_motifs.json");
            tokio::fs::write(&stop_motifs_path, serialized).await
                .map_err(|e| ValknutError::io("Failed to save stop motifs cache".to_string(), e))?;
        }
        
        // Save hub patterns
        let hub_patterns: Vec<_> = hub_suppressor.hub_patterns.iter().cloned().collect();
        let serialized = serde_json::to_string_pretty(&hub_patterns)?;
        let hub_patterns_path = self.cache_dir.join("hub_patterns.json");
        tokio::fs::write(&hub_patterns_path, serialized).await
            .map_err(|e| ValknutError::io("Failed to save hub patterns cache".to_string(), e))?;
        
        Ok(())
    }
    
    /// Load cache from disk
    async fn load_cache(
        &self,
    ) -> Result<(StopMotifDatabase, Vec<HubPattern>)> {
        let stop_motifs_path = self.cache_dir.join("stop_motifs.json");
        let hub_patterns_path = self.cache_dir.join("hub_patterns.json");
        
        // Load stop motifs
        let stop_motifs = if stop_motifs_path.exists() {
            let content = tokio::fs::read_to_string(&stop_motifs_path).await
                .map_err(|e| ValknutError::io("Failed to read stop motifs cache".to_string(), e))?;
            serde_json::from_str(&content)?
        } else {
            StopMotifDatabase::new()
        };
        
        // Load hub patterns
        let hub_patterns = if hub_patterns_path.exists() {
            let content = tokio::fs::read_to_string(&hub_patterns_path).await
                .map_err(|e| ValknutError::io("Failed to read hub patterns cache".to_string(), e))?;
            serde_json::from_str(&content)?
        } else {
            Vec::new()
        };
        
        Ok((stop_motifs, hub_patterns))
    }
}

/// Configuration for boilerplate learning
#[derive(Debug, Clone)]
pub struct BoilerplateLearningConfig {
    /// Minimum support threshold for frequent patterns
    pub min_support_threshold: usize,
    
    /// Percentile for identifying stop motifs (e.g., 0.75 for top 0.75%)
    pub stop_motif_percentile: f64,
    
    /// Down-weighting factor for stop motifs (0.8 = 80% down-weighting)
    pub stop_motif_downweight: f64,
    
    /// Hub suppression threshold (0.6 = patterns appearing in >60% of files)
    pub hub_suppression_threshold: f64,
    
    /// Refresh interval in days
    pub refresh_interval_days: i64,
    
    /// Enable automatic cache refresh
    pub auto_refresh_enabled: bool,
}

impl Default for BoilerplateLearningConfig {
    fn default() -> Self {
        Self {
            min_support_threshold: 10,
            stop_motif_percentile: 0.75,
            stop_motif_downweight: 0.9, // 90% down-weighting
            hub_suppression_threshold: 0.6,
            refresh_interval_days: 7, // Weekly refresh
            auto_refresh_enabled: true,
        }
    }
}

/// Learning report
#[derive(Debug)]
pub struct LearningReport {
    pub shingles_analyzed: usize,
    pub motifs_analyzed: usize,
    pub stop_shingles_identified: usize,
    pub stop_motifs_identified: usize,
    pub learning_duration: chrono::Duration,
}

impl LearningReport {
    fn new() -> Self {
        Self {
            shingles_analyzed: 0,
            motifs_analyzed: 0,
            stop_shingles_identified: 0,
            stop_motifs_identified: 0,
            learning_duration: Duration::zero(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio;
    
    #[tokio::test]
    async fn test_frequent_pattern_miner() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create test source files
        let test_code = r#"
            fn test_function() {
                println!("hello world");
                println!("hello world");
                if x > 0 {
                    println!("positive");
                }
            }
        "#;
        
        let test_file = temp_path.join("test.rs");
        tokio::fs::write(&test_file, test_code).await.unwrap();
        
        let miner = FrequentPatternMiner::new(1);
        let shingles = miner.mine_shingles(temp_path).await.unwrap();
        
        assert!(!shingles.is_empty());
    }
    
    #[tokio::test]
    async fn test_boilerplate_learning_system() {
        let config = BoilerplateLearningConfig::default();
        let mut system = BoilerplateLearningSystem::new(config);
        
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create test files
        let test_codes = [
            "fn test() { println!(\"test\"); }",
            "fn test2() { println!(\"test\"); }",
            "fn test3() { println!(\"different\"); }",
        ];
        
        for (i, code) in test_codes.iter().enumerate() {
            let file_path = temp_path.join(format!("test_{}.rs", i));
            tokio::fs::write(&file_path, code).await.unwrap();
        }
        
        let report = system.learn_from_codebase(temp_path).await.unwrap();
        
        assert!(report.shingles_analyzed > 0);
        assert!(report.learning_duration.num_milliseconds() >= 0);
    }
    
    #[test]
    fn test_hub_suppressor() {
        let suppressor = HubSuppressor::new();
        
        // Test default patterns with tree-sitter analysis
        assert!(suppressor.is_hub_pattern("log.info(\"test\");", "test.py"));
        assert!(suppressor.is_hub_pattern("counter.increment();", "test.js"));
        assert!(suppressor.is_hub_pattern("router.get(\"/api\");", "test.ts"));
        
        // Test non-hub patterns
        assert!(!suppressor.is_hub_pattern("calculate_result(x, y);", "test.py"));
    }
    
    #[test]
    fn test_stop_motif_database() {
        let mut db = StopMotifDatabase::new();
        
        let mut shingles = HashMap::new();
        shingles.insert("common pattern".to_string(), 100);
        shingles.insert("rare pattern".to_string(), 1);
        
        db.update_stop_shingles(shingles);
        
        assert_eq!(db.max_shingle_frequency, 100);
        assert!(db.stop_shingles.contains_key("common pattern"));
        assert!(db.stop_shingles.contains_key("rare pattern"));
    }
}