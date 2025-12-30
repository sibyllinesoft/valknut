//! Cache implementation with support for stop-motifs and other analysis caches.

pub mod language_adapters;
pub mod types;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
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

impl Default for CacheRefreshPolicy {
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

/// Pattern Mining Engine for extracting frequent k-grams and PDG motifs
#[derive(Debug)]
pub struct PatternMiner {
    /// Refresh policy with mining parameters
    policy: CacheRefreshPolicy,

    /// K-gram frequency map
    kgram_frequencies: HashMap<String, usize>,

    /// PDG motif frequency map
    motif_frequencies: HashMap<String, usize>,

    /// Total documents (functions) processed
    total_documents: usize,
}

impl PatternMiner {
    /// Create a new pattern miner
    pub fn new(policy: CacheRefreshPolicy) -> Self {
        Self {
            policy,
            kgram_frequencies: HashMap::new(),
            motif_frequencies: HashMap::new(),
            total_documents: 0,
        }
    }

    /// Mine stop-motifs from the entire codebase
    pub fn mine_stop_motifs(&mut self, codebase_info: &CodebaseInfo) -> Result<StopMotifCache> {
        let start_time = SystemTime::now();

        tracing::info!(
            "Mining patterns from {} functions",
            codebase_info.functions.len()
        );

        // Phase 1: Extract all k-grams and motifs from functions
        self.extract_all_patterns(codebase_info)?;

        // Phase 2: Calculate IDF scores
        let idf_scores = self.calculate_idf_scores();

        // Phase 3: Select top patterns as stop-motifs
        let stop_motifs = self.select_stop_motifs(&idf_scores)?;

        let mining_duration = start_time
            .elapsed()
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_millis() as u64;

        let mining_stats = MiningStats {
            functions_analyzed: codebase_info.functions.len(),
            unique_kgrams_found: self.kgram_frequencies.len(),
            unique_motifs_found: self.motif_frequencies.len(),
            ast_patterns_found: 0,         // Will be updated by AST mining
            ast_node_types_found: 0,       // Will be updated by AST mining
            ast_subtree_patterns_found: 0, // Will be updated by AST mining
            stop_motifs_selected: stop_motifs.len(),
            percentile_threshold: self.policy.stop_motif_percentile,
            mining_duration_ms: mining_duration,
            languages_processed: HashSet::new(), // Will be updated by AST mining
        };

        tracing::info!(
            "Pattern mining complete: {} unique k-grams, {} unique motifs, {} stop-motifs selected",
            mining_stats.unique_kgrams_found,
            mining_stats.unique_motifs_found,
            mining_stats.stop_motifs_selected
        );

        // Mine AST patterns using the new AST Stop-Motif Miner
        let mut ast_miner = AstStopMotifMiner::new();
        let ast_patterns = ast_miner
            .mine_ast_stop_motifs(&codebase_info.functions)
            .unwrap_or_else(|e| {
                eprintln!("Failed to mine AST patterns: {:?}", e);
                Vec::new()
            });

        // Update mining stats with AST pattern information
        let mut updated_mining_stats = mining_stats;
        updated_mining_stats.ast_patterns_found = ast_patterns.len();
        updated_mining_stats.ast_node_types_found = ast_patterns
            .iter()
            .filter(|p| matches!(p.category, AstPatternCategory::NodeType))
            .count();
        updated_mining_stats.ast_subtree_patterns_found = ast_patterns
            .iter()
            .filter(|p| matches!(p.category, AstPatternCategory::SubtreePattern))
            .count();
        updated_mining_stats.languages_processed =
            ast_patterns.iter().map(|p| p.language.clone()).collect();

        Ok(StopMotifCache {
            version: 1,
            k_gram_size: self.policy.k_gram_size,
            token_grams: stop_motifs
                .clone()
                .into_iter()
                .filter(|e| e.category == PatternCategory::TokenGram)
                .collect(),
            pdg_motifs: stop_motifs
                .into_iter()
                .filter(|e| e.category != PatternCategory::TokenGram)
                .collect(),
            ast_patterns,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            codebase_signature: self.compute_signature(codebase_info),
            mining_stats: updated_mining_stats,
        })
    }

    /// Extract all patterns from the codebase
    fn extract_all_patterns(&mut self, codebase_info: &CodebaseInfo) -> Result<()> {
        // Process functions in parallel for performance
        let kgram_freq: HashMap<String, usize> = codebase_info
            .functions
            .par_iter()
            .map(|func| self.extract_function_kgrams(func))
            .reduce(HashMap::new, |mut acc, freq_map| {
                for (kgram, count) in freq_map {
                    *acc.entry(kgram).or_insert(0) += count;
                }
                acc
            });

        let motif_freq: HashMap<String, usize> = codebase_info
            .functions
            .par_iter()
            .map(|func| self.extract_function_motifs(func))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .reduce(|mut acc, freq_map| {
                for (motif, count) in freq_map {
                    *acc.entry(motif).or_insert(0) += count;
                }
                acc
            })
            .unwrap_or_default();

        self.kgram_frequencies = kgram_freq;
        self.motif_frequencies = motif_freq;
        self.total_documents = codebase_info.functions.len();

        Ok(())
    }

    /// Extract k-grams from a single function
    fn extract_function_kgrams(&self, func: &FunctionInfo) -> HashMap<String, usize> {
        let mut kgram_freq = HashMap::new();

        // Tokenize the source code
        let tokens: Vec<String> = func
            .source_code
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(|token| self.normalize_token(token))
            .collect();

        // Generate k-grams
        if tokens.len() >= self.policy.k_gram_size {
            for window in tokens.windows(self.policy.k_gram_size) {
                let kgram = window.join(" ");
                *kgram_freq.entry(kgram).or_insert(0) += 1;
            }
        }

        kgram_freq
    }

    /// Extract PDG motifs from a single function
    fn extract_function_motifs(&self, func: &FunctionInfo) -> Result<HashMap<String, usize>> {
        let mut motif_freq = HashMap::new();

        // Use a simplified motif extractor (in practice, would integrate with PdgMotifAnalyzer)
        let motifs = self.extract_simplified_motifs(&func.source_code)?;

        for motif in motifs {
            let motif_key = format!("{}:{}", motif.category_str(), motif.pattern);
            *motif_freq.entry(motif_key).or_insert(0) += 1;
        }

        Ok(motif_freq)
    }

    /// Pattern definitions for motif extraction: (patterns, pattern_name, category)
    const MOTIF_PATTERNS: &'static [(&'static [&'static str], &'static str, PatternCategory)] = &[
        (&["if ", "else"], "branch", PatternCategory::ControlFlow),
        (
            &["for ", "while ", "loop"],
            "loop",
            PatternCategory::ControlFlow,
        ),
        (
            &["Vec::", "HashMap::", "HashSet::"],
            "collection",
            PatternCategory::DataStructure,
        ),
        (
            &["println!", "eprintln!", "dbg!"],
            "debug_print",
            PatternCategory::Boilerplate,
        ),
        (
            &["unwrap()", "expect("],
            "error_unwrap",
            PatternCategory::Boilerplate,
        ),
    ];

    /// Extract simplified structural motifs from source code
    fn extract_simplified_motifs(&self, source_code: &str) -> Result<Vec<SimplifiedMotif>> {
        let mut motifs = Vec::new();

        for line in source_code.lines() {
            let line = line.trim();

            // Check pattern-based motifs
            for &(patterns, name, category) in Self::MOTIF_PATTERNS {
                if patterns.iter().any(|p| line.contains(p)) {
                    motifs.push(SimplifiedMotif {
                        pattern: name.to_string(),
                        category,
                    });
                }
            }

            // Assignment pattern (has special exclusion logic)
            if line.contains('=') && !line.contains("==") && !line.contains("!=") {
                motifs.push(SimplifiedMotif {
                    pattern: "assign".to_string(),
                    category: PatternCategory::Assignment,
                });
            }

            // Function call pattern (has special exclusion logic)
            if line.contains('(') && !line.trim_start().starts_with("//") {
                motifs.push(SimplifiedMotif {
                    pattern: "call".to_string(),
                    category: PatternCategory::FunctionCall,
                });
            }
        }

        Ok(motifs)
    }

    /// Calculate IDF scores for all patterns
    fn calculate_idf_scores(&self) -> HashMap<String, f64> {
        let mut idf_scores = HashMap::new();

        // Calculate IDF for k-grams
        for (kgram, &doc_freq) in &self.kgram_frequencies {
            let idf = if doc_freq > 0 && self.total_documents > 0 {
                (self.total_documents as f64 / doc_freq as f64).ln()
            } else {
                0.0
            };
            idf_scores.insert(format!("kgram:{}", kgram), idf);
        }

        // Calculate IDF for motifs
        for (motif, &doc_freq) in &self.motif_frequencies {
            let idf = if doc_freq > 0 && self.total_documents > 0 {
                (self.total_documents as f64 / doc_freq as f64).ln()
            } else {
                0.0
            };
            idf_scores.insert(format!("motif:{}", motif), idf);
        }

        idf_scores
    }

    /// Select stop-motifs based on frequency (top percentile)
    fn select_stop_motifs(&self, idf_scores: &HashMap<String, f64>) -> Result<Vec<StopMotifEntry>> {
        let mut all_patterns: Vec<PatternCandidate> = Vec::new();

        // Collect k-gram candidates
        for (kgram, &support) in &self.kgram_frequencies {
            let key = format!("kgram:{}", kgram);
            let idf = idf_scores.get(&key).copied().unwrap_or(0.0);

            all_patterns.push(PatternCandidate {
                pattern: kgram.clone(),
                support,
                idf_score: idf,
                category: PatternCategory::TokenGram,
            });
        }

        // Collect motif candidates
        for (motif, &support) in &self.motif_frequencies {
            let key = format!("motif:{}", motif);
            let idf = idf_scores.get(&key).copied().unwrap_or(0.0);

            let category = self.categorize_motif(motif);
            all_patterns.push(PatternCandidate {
                pattern: motif.clone(),
                support,
                idf_score: idf,
                category,
            });
        }

        // Sort by support (frequency) descending
        all_patterns.sort_by(|a, b| b.support.cmp(&a.support));

        // Select top percentile
        let selection_count = ((all_patterns.len() as f64) * self.policy.stop_motif_percentile
            / 100.0)
            .ceil() as usize;
        let selection_count = selection_count.max(1).min(all_patterns.len());

        let stop_motifs = all_patterns
            .into_iter()
            .take(selection_count)
            .map(|candidate| StopMotifEntry {
                pattern: candidate.pattern,
                support: candidate.support,
                idf_score: candidate.idf_score,
                weight_multiplier: self.policy.weight_multiplier,
                category: candidate.category,
            })
            .collect();

        Ok(stop_motifs)
    }

    /// Normalize a token for consistent analysis
    fn normalize_token(&self, token: &str) -> String {
        // Preserve control flow keywords and important language constructs
        match token {
            // Control flow keywords - preserve these for pattern detection
            "if" | "else" | "for" | "while" | "loop" | "match" | "switch" | "case" | "break"
            | "continue" | "return" | "yield" | "await" | "try" | "catch" | "finally" | "throw"
            | "with" => token.to_string(),

            // Function/class keywords - preserve for structural patterns
            "fn" | "function" | "def" | "class" | "struct" | "enum" | "trait" | "interface"
            | "type" | "let" | "var" | "const" | "mut" | "pub" | "public" | "private"
            | "protected" | "static" => token.to_string(),

            // Operators - preserve common ones
            "==" | "!=" | "<=" | ">=" | "&&" | "||" | "+=" | "-=" | "*=" | "/=" | "=>" | "->"
            | "::" | "." | ";" | "," | "(" | ")" | "{" | "}" | "[" | "]" | "<" | ">" => {
                token.to_string()
            }

            // Everything else gets normalized
            _ => {
                // Simple normalization - could be more sophisticated
                if token.parse::<f64>().is_ok() {
                    if token.contains('.') {
                        "FLOAT_LIT".to_string()
                    } else {
                        "INT_LIT".to_string()
                    }
                } else if (token.starts_with('"') && token.ends_with('"'))
                    || (token.starts_with('\'') && token.ends_with('\''))
                {
                    "STR_LIT".to_string()
                } else if token.len() < 20
                    && token.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && token.chars().any(|c| c.is_lowercase())
                {
                    "LOCAL_VAR".to_string()
                } else {
                    token.to_string()
                }
            }
        }
    }

    /// Categorize a motif based on its name
    fn categorize_motif(&self, motif: &str) -> PatternCategory {
        if motif.contains("branch") || motif.contains("if") {
            PatternCategory::ControlFlow
        } else if motif.contains("loop") || motif.contains("for") || motif.contains("while") {
            PatternCategory::ControlFlow
        } else if motif.contains("assign") {
            PatternCategory::Assignment
        } else if motif.contains("call") {
            PatternCategory::FunctionCall
        } else if motif.contains("collection") || motif.contains("Vec") || motif.contains("HashMap")
        {
            PatternCategory::DataStructure
        } else if motif.contains("debug_print") || motif.contains("unwrap") {
            PatternCategory::Boilerplate
        } else {
            PatternCategory::Boilerplate
        }
    }

    /// Compute signature for codebase
    fn compute_signature(&self, codebase_info: &CodebaseInfo) -> String {
        let mut hasher = Sha256::new();
        hasher.update(codebase_info.functions.len().to_be_bytes());
        hasher.update(codebase_info.total_lines.to_be_bytes());
        format!("{:x}", hasher.finalize())
    }
}

/// Simplified motif for pattern extraction
#[derive(Debug, Clone)]
struct SimplifiedMotif {
    pattern: String,
    category: PatternCategory,
}

impl SimplifiedMotif {
    fn category_str(&self) -> &'static str {
        match self.category {
            PatternCategory::TokenGram => "token",
            PatternCategory::ControlFlow => "control",
            PatternCategory::Assignment => "assign",
            PatternCategory::FunctionCall => "call",
            PatternCategory::DataStructure => "data",
            PatternCategory::Boilerplate => "boiler",
            PatternCategory::AstNodeType => "ast_node",
            PatternCategory::AstSubtree => "ast_subtree",
            PatternCategory::AstTokenSequence => "ast_token",
        }
    }
}

/// Pattern candidate for stop-motif selection
#[derive(Debug, Clone)]
struct PatternCandidate {
    pattern: String,
    support: usize,
    idf_score: f64,
    category: PatternCategory,
}

/// Phase 3: AST Stop-Motif Miner using tree-sitter analysis
pub struct AstStopMotifMiner {
    /// Language adapters for AST parsing
    language_adapters: HashMap<String, Box<dyn LanguageAdapter>>,

    /// Pattern extractor for AST analysis
    pattern_extractor: AstPatternExtractor,

    /// Frequency thresholds for pattern selection
    frequency_thresholds: PatternThresholds,
}

impl AstStopMotifMiner {
    /// Create a new AST stop-motif miner
    pub fn new() -> Self {
        let mut language_adapters: HashMap<String, Box<dyn LanguageAdapter>> = HashMap::new();

        // Initialize language adapters
        if let Ok(python_adapter) = PythonLanguageAdapter::new() {
            language_adapters.insert("python".to_string(), Box::new(python_adapter));
        }

        if let Ok(js_adapter) = JavaScriptLanguageAdapter::new() {
            language_adapters.insert("javascript".to_string(), Box::new(js_adapter));
        }

        if let Ok(ts_adapter) = TypeScriptLanguageAdapter::new() {
            language_adapters.insert("typescript".to_string(), Box::new(ts_adapter));
        }

        if let Ok(rust_adapter) = RustLanguageAdapter::new() {
            language_adapters.insert("rust".to_string(), Box::new(rust_adapter));
        }

        if let Ok(go_adapter) = GoLanguageAdapter::new() {
            language_adapters.insert("go".to_string(), Box::new(go_adapter));
        }

        let config = AstExtractionConfig::default();
        let thresholds = PatternThresholds::default();

        Self {
            language_adapters,
            pattern_extractor: AstPatternExtractor::new(config),
            frequency_thresholds: thresholds,
        }
    }

    /// Mine AST stop-motifs from codebase functions
    pub fn mine_ast_stop_motifs(
        &mut self,
        functions: &[FunctionInfo],
    ) -> Result<Vec<AstStopMotifEntry>> {
        let start_time = std::time::Instant::now();
        let mut all_patterns = Vec::new();
        let mut languages_processed = HashSet::new();

        // Extract patterns from all functions
        for function in functions {
            let language = self.detect_language(&function.file_path);

            if let Some(adapter) = self.language_adapters.get_mut(&language) {
                languages_processed.insert(language.clone());

                // Parse the function source code
                match adapter.parse_source(&function.source_code, &function.file_path) {
                    Ok(parse_index) => {
                        // Extract AST patterns
                        match adapter.extract_ast_patterns(&parse_index, &function.source_code) {
                            Ok(patterns) => {
                                all_patterns.extend(patterns);
                            }
                            Err(e) => {
                                eprintln!(
                                    "Failed to extract AST patterns from {}: {:?}",
                                    function.id, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to parse source code for {}: {:?}", function.id, e);
                    }
                }
            }
        }

        // Analyze pattern frequencies
        self.pattern_extractor
            .analyze_pattern_frequencies(&all_patterns);

        // Select stop-motifs based on frequency thresholds
        let stop_motifs = self.select_stop_motifs(&all_patterns)?;

        let duration = start_time.elapsed();
        println!(
            "AST stop-motif mining completed in {:?}ms",
            duration.as_millis()
        );
        println!(
            "Found {} AST patterns, selected {} as stop-motifs",
            all_patterns.len(),
            stop_motifs.len()
        );
        println!("Languages processed: {:?}", languages_processed);

        Ok(stop_motifs)
    }

    /// Detect programming language from file path
    fn detect_language(&self, file_path: &str) -> String {
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

    /// Select stop-motifs based on frequency analysis
    fn select_stop_motifs(&self, patterns: &[AstPattern]) -> Result<Vec<AstStopMotifEntry>> {
        let mut stop_motifs = Vec::new();

        // Calculate pattern frequencies by type
        let mut pattern_frequencies: HashMap<String, usize> = HashMap::new();
        for pattern in patterns {
            *pattern_frequencies.entry(pattern.id.clone()).or_insert(0) += 1;
        }

        // Sort patterns by frequency
        let mut frequency_pairs: Vec<(String, usize)> = pattern_frequencies.into_iter().collect();
        frequency_pairs.sort_by(|a, b| b.1.cmp(&a.1));

        let total_patterns = frequency_pairs.len();

        // Select top percentile patterns as stop-motifs
        for (i, (pattern_id, support)) in frequency_pairs.iter().enumerate() {
            if let Some(pattern) = patterns.iter().find(|p| &p.id == pattern_id) {
                let percentile_threshold = match pattern.pattern_type {
                    AstPatternType::NodeType => self.frequency_thresholds.node_type_percentile,
                    AstPatternType::SubtreePattern => self.frequency_thresholds.subtree_percentile,
                    AstPatternType::TokenSequence => {
                        self.frequency_thresholds.token_sequence_percentile
                    }
                    AstPatternType::ControlFlowPattern => {
                        self.frequency_thresholds.subtree_percentile
                    }
                    AstPatternType::FrameworkPattern => {
                        self.frequency_thresholds.subtree_percentile
                    }
                };

                // Calculate which percentile this pattern falls into
                let pattern_rank = i + 1;

                let pattern_percentile = 1.0 - (pattern_rank as f64 / total_patterns as f64);

                if pattern_percentile >= percentile_threshold
                    && *support >= self.pattern_extractor.config.min_support
                {
                    // Calculate IDF score
                    let total_functions = patterns.len();
                    let idf_score = (total_functions as f64 / *support as f64).ln();

                    if idf_score >= self.frequency_thresholds.min_idf_score {
                        let category = match pattern.pattern_type {
                            AstPatternType::NodeType => AstPatternCategory::NodeType,
                            AstPatternType::SubtreePattern => AstPatternCategory::SubtreePattern,
                            AstPatternType::TokenSequence => AstPatternCategory::TokenSequence,
                            AstPatternType::ControlFlowPattern => {
                                AstPatternCategory::ControlFlowPattern
                            }
                            AstPatternType::FrameworkPattern => {
                                AstPatternCategory::FrameworkPattern
                            }
                        };

                        let stop_motif = AstStopMotifEntry {
                            pattern: pattern.id.clone(),
                            support: *support,
                            idf_score,
                            weight_multiplier: 0.2, // Common weight for stop-motifs
                            category,
                            language: pattern.language.clone(),
                            metadata: pattern.metadata.clone(),
                        };

                        stop_motifs.push(stop_motif);
                    }
                }
            }
        }

        Ok(stop_motifs)
    }
}

#[derive(Debug, Default)]
pub struct Cache;

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests;
