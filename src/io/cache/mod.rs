//! Cache implementation with support for stop-motifs and other analysis caches.

mod ast_stop_motif_miner;
pub mod language_adapters;
mod pattern_miner;
pub mod types;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::errors::{Result, ValknutError, ValknutResultExt};

// Re-export types from submodules
pub use language_adapters::{
    GoLanguageAdapter, JavaScriptLanguageAdapter, LanguageAdapter, PythonLanguageAdapter,
    RustLanguageAdapter, TypeScriptLanguageAdapter,
};
pub use types::{
    AstExtractionConfig, AstPattern, AstPatternExtractor, AstPatternType, PatternThresholds,
};

// Re-export miners from submodules
pub use ast_stop_motif_miner::AstStopMotifMiner;
pub use pattern_miner::PatternMiner;

/// Phase 3 Stop-Motifs Cache for automatic boilerplate pattern detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopMotifCache {
    /// Cache format version for migration support
    pub version: u32,

    /// K-gram size used for token analysis
    pub k_gram_size: usize,

    /// Token k-grams identified as common boilerplate
    pub token_grams: Vec<StopMotifEntry>,

    /// PDG motifs identified as common patterns
    pub pdg_motifs: Vec<StopMotifEntry>,

    /// AST-based patterns from tree-sitter analysis
    pub ast_patterns: Vec<AstStopMotifEntry>,

    /// Last cache update timestamp
    pub last_updated: u64, // Unix timestamp

    /// Codebase signature for invalidation detection
    pub codebase_signature: String,

    /// Statistics about the mining process
    pub mining_stats: MiningStats,
}

/// Individual stop-motif entry with frequency and weight information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopMotifEntry {
    /// Pattern string (k-gram or motif label)
    pub pattern: String,

    /// Support count (frequency across codebase)
    pub support: usize,

    /// IDF score for weight calculation
    pub idf_score: f64,

    /// Applied weight multiplier (typically 0.2 for stop-motifs)
    pub weight_multiplier: f64,

    /// Pattern category for analysis
    pub category: PatternCategory,
}

/// Category of pattern for stop-motif classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PatternCategory {
    TokenGram,
    ControlFlow,
    Assignment,
    FunctionCall,
    DataStructure,
    Boilerplate,
    // AST-specific categories
    AstNodeType,
    AstSubtree,
    AstTokenSequence,
}

/// AST-based stop-motif entry with tree-sitter specific information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstStopMotifEntry {
    /// Pattern identifier (node type, subtree signature, token sequence)
    pub pattern: String,

    /// Support count across codebase
    pub support: usize,

    /// IDF score for this pattern
    pub idf_score: f64,

    /// Weight multiplier for denoising
    pub weight_multiplier: f64,

    /// Category of AST pattern
    pub category: AstPatternCategory,

    /// Language where pattern was found
    pub language: String,

    /// Optional metadata about the pattern
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Categories of AST patterns for classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AstPatternCategory {
    /// Common AST node types (decorator_list, import_statement)
    NodeType,

    /// Structural subtree patterns (call_expression->member_access)
    SubtreePattern,

    /// Token sequence patterns frequently appearing
    TokenSequence,

    /// Control flow patterns (if/else, loops)
    ControlFlowPattern,

    /// Framework-specific boilerplate patterns
    FrameworkPattern,
}

/// Statistics from the pattern mining process
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MiningStats {
    /// Total functions analyzed
    pub functions_analyzed: usize,

    /// Total unique k-grams found
    pub unique_kgrams_found: usize,

    /// Total unique PDG motifs found
    pub unique_motifs_found: usize,

    /// Total AST patterns found
    pub ast_patterns_found: usize,

    /// AST node types discovered
    pub ast_node_types_found: usize,

    /// AST subtree patterns discovered
    pub ast_subtree_patterns_found: usize,

    /// Number of patterns selected as stop-motifs
    pub stop_motifs_selected: usize,

    /// Top percentile threshold used
    pub percentile_threshold: f64,

    /// Mining duration in milliseconds
    pub mining_duration_ms: u64,

    /// Languages processed
    pub languages_processed: HashSet<String>,
}

/// Stop-Motifs Cache Manager with refresh and invalidation logic
#[derive(Debug)]
pub struct StopMotifCacheManager {
    /// Cache directory path
    cache_dir: PathBuf,

    /// In-memory cache
    cache: Arc<RwLock<Option<StopMotifCache>>>,

    /// Refresh policy configuration
    refresh_policy: CacheRefreshPolicy,

    /// Thread-safe mining mutex
    mining_mutex: Arc<Mutex<()>>,
}

/// Cache refresh policy configuration
#[derive(Debug, Clone)]
pub struct CacheRefreshPolicy {
    /// Maximum cache age in days
    pub max_age_days: u64,

    /// Codebase change threshold for refresh (percentage)
    pub change_threshold_percent: f64,

    /// Stop-motif selection percentile (top X%)
    pub stop_motif_percentile: f64,

    /// Default weight multiplier for stop-motifs
    pub weight_multiplier: f64,

    /// K-gram size for token analysis
    pub k_gram_size: usize,
}

/// Default implementation for [`CacheRefreshPolicy`].
impl Default for CacheRefreshPolicy {
    /// Returns the default cache refresh policy.
    fn default() -> Self {
        Self {
            max_age_days: 7,
            change_threshold_percent: 5.0,
            stop_motif_percentile: 0.5, // Top 0.5% by support
            weight_multiplier: 0.2,
            k_gram_size: 9,
        }
    }
}

/// Factory, caching, and mining methods for [`StopMotifCacheManager`].
impl StopMotifCacheManager {
    /// Create a new stop-motif cache manager
    pub fn new<P: AsRef<Path>>(cache_dir: P, refresh_policy: CacheRefreshPolicy) -> Self {
        let cache_dir = cache_dir.as_ref().to_path_buf();

        Self {
            cache_dir,
            cache: Arc::new(RwLock::new(None)),
            refresh_policy,
            mining_mutex: Arc::new(Mutex::new(())),
        }
    }

    /// Get or create the stop-motif cache
    pub fn get_cache(&self, codebase_info: &CodebaseInfo) -> Result<Arc<StopMotifCache>> {
        // Check if we have a valid cached version
        if let Some(cache) = self.get_valid_cache(codebase_info)? {
            return Ok(Arc::new(cache));
        }

        // Need to refresh/create cache
        self.refresh_cache(codebase_info)
    }

    /// Check if we have a valid cached version
    fn get_valid_cache(&self, codebase_info: &CodebaseInfo) -> Result<Option<StopMotifCache>> {
        let cache_path = self.get_cache_path();

        // Check if cache file exists
        if !cache_path.exists() {
            tracing::debug!("Cache file does not exist: {}", cache_path.display());
            return Ok(None);
        }

        // Load existing cache
        let cache = self.load_cache(&cache_path)?;

        // Validate cache age
        let cache_age = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_generic_err("getting system time")?
            .as_secs()
            - cache.last_updated;

        let max_age_seconds = self.refresh_policy.max_age_days * 24 * 60 * 60;
        if cache_age > max_age_seconds {
            tracing::info!(
                "Cache expired: {} days old (max: {} days)",
                cache_age / (24 * 60 * 60),
                self.refresh_policy.max_age_days
            );
            return Ok(None);
        }

        // Validate codebase signature
        let current_signature = self.compute_codebase_signature(codebase_info);
        if cache.codebase_signature != current_signature {
            let change_percent =
                self.estimate_change_percentage(&cache.codebase_signature, &current_signature);
            if change_percent > self.refresh_policy.change_threshold_percent {
                tracing::info!(
                    "Codebase changed significantly: {:.1}% (threshold: {:.1}%)",
                    change_percent,
                    self.refresh_policy.change_threshold_percent
                );
                return Ok(None);
            }
        }

        tracing::debug!("Using valid cached stop-motifs");
        Ok(Some(cache))
    }

    /// Refresh the cache by mining new patterns
    fn refresh_cache(&self, codebase_info: &CodebaseInfo) -> Result<Arc<StopMotifCache>> {
        // Ensure only one thread mines at a time
        let _mining_lock = self.mining_mutex.lock().unwrap();

        tracing::info!(
            "Refreshing stop-motifs cache for {} functions",
            codebase_info.functions.len()
        );
        let start_time = SystemTime::now();

        // Mine patterns from entire codebase
        let mut miner = PatternMiner::new(self.refresh_policy.clone());
        let cache = miner.mine_stop_motifs(codebase_info)?;

        // Save cache atomically
        self.save_cache(&cache)?;

        // Update in-memory cache
        *self.cache.write().unwrap() = Some(cache.clone());

        let mining_duration = start_time
            .elapsed()
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64;

        tracing::info!(
            "Stop-motifs cache refreshed in {}ms: {} token grams, {} motifs",
            mining_duration,
            cache.token_grams.len(),
            cache.pdg_motifs.len()
        );

        Ok(Arc::new(cache))
    }

    /// Load cache from disk
    fn load_cache(&self, cache_path: &Path) -> Result<StopMotifCache> {
        let content = fs::read_to_string(cache_path).map_err(|e| {
            ValknutError::io(
                format!("Failed to read cache file: {}", cache_path.display()),
                e,
            )
        })?;

        serde_json::from_str(&content).map_json_err("cache file content")
    }

    /// Save cache to disk atomically
    fn save_cache(&self, cache: &StopMotifCache) -> Result<()> {
        // Ensure cache directory exists
        fs::create_dir_all(&self.cache_dir).map_err(|e| {
            ValknutError::io(
                format!(
                    "Failed to create cache directory: {}",
                    self.cache_dir.display()
                ),
                e,
            )
        })?;

        let cache_path = self.get_cache_path();
        let temp_path = cache_path.with_extension("tmp");

        // Write to temporary file first
        let content = serde_json::to_string_pretty(cache).map_json_err("cache serialization")?;

        fs::write(&temp_path, content).map_err(|e| {
            ValknutError::io(
                format!("Failed to write cache file: {}", temp_path.display()),
                e,
            )
        })?;

        // Atomic rename
        fs::rename(&temp_path, &cache_path).map_err(|e| {
            ValknutError::io(
                format!("Failed to rename cache file: {}", cache_path.display()),
                e,
            )
        })?;

        Ok(())
    }

    /// Get the cache file path
    fn get_cache_path(&self) -> PathBuf {
        self.cache_dir.join("stop_motifs.v1.json")
    }

    /// Compute codebase signature for change detection
    fn compute_codebase_signature(&self, codebase_info: &CodebaseInfo) -> String {
        let mut hasher = Sha256::new();

        // Hash function count and total lines
        hasher.update(codebase_info.functions.len().to_be_bytes());
        hasher.update(codebase_info.total_lines.to_be_bytes());

        // Hash file paths and sizes (for structure changes)
        let mut file_info: Vec<_> = codebase_info.file_info.iter().collect();
        file_info.sort_by_key(|&(path, _)| path);

        for (path, info) in file_info {
            hasher.update(path.as_bytes());
            hasher.update(info.line_count.to_be_bytes());
            hasher.update(&info.content_hash);
        }

        format!("{:x}", hasher.finalize())
    }

    /// Estimate change percentage between signatures
    fn estimate_change_percentage(&self, old_sig: &str, new_sig: &str) -> f64 {
        if old_sig == new_sig {
            return 0.0;
        }

        // Simple heuristic: if signatures differ completely, assume significant change
        // In practice, could implement more sophisticated delta analysis
        50.0
    }
}

/// Information about the codebase for pattern mining
#[derive(Debug, Clone)]
pub struct CodebaseInfo {
    /// All functions in the codebase
    pub functions: Vec<FunctionInfo>,

    /// Total lines of code
    pub total_lines: usize,

    /// File-level information for signature computation
    pub file_info: HashMap<String, FileInfo>,
}

/// Information about a function for pattern analysis
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// Function identifier
    pub id: String,

    /// Source code
    pub source_code: String,

    /// File path
    pub file_path: String,

    /// Line count
    pub line_count: usize,
}

/// File-level information for change detection
#[derive(Debug, Clone)]
pub struct FileInfo {
    /// Number of lines in file
    pub line_count: usize,

    /// Hash of file content for change detection
    pub content_hash: Vec<u8>,
}

/// Empty cache placeholder.
#[derive(Debug, Default)]
pub struct Cache;

/// Factory methods for [`Cache`].
impl Cache {
    /// Creates a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests;
