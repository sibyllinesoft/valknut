//! Cache implementation with support for stop-motifs and other analysis caches.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::core::errors::{Result, ValknutError, ValknutResultExt};
// Note: PdgMotif and MotifCategory will be imported when needed

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

    /// Extract simplified structural motifs from source code
    fn extract_simplified_motifs(&self, source_code: &str) -> Result<Vec<SimplifiedMotif>> {
        let mut motifs = Vec::new();

        for line in source_code.lines() {
            let line = line.trim();

            // Control flow patterns
            if line.contains("if ") || line.contains("else") {
                motifs.push(SimplifiedMotif {
                    pattern: "branch".to_string(),
                    category: PatternCategory::ControlFlow,
                });
            }

            if line.contains("for ") || line.contains("while ") || line.contains("loop") {
                motifs.push(SimplifiedMotif {
                    pattern: "loop".to_string(),
                    category: PatternCategory::ControlFlow,
                });
            }

            // Assignment patterns
            if line.contains('=') && !line.contains("==") && !line.contains("!=") {
                motifs.push(SimplifiedMotif {
                    pattern: "assign".to_string(),
                    category: PatternCategory::Assignment,
                });
            }

            // Function call patterns
            if line.contains('(') && !line.trim_start().starts_with("//") {
                motifs.push(SimplifiedMotif {
                    pattern: "call".to_string(),
                    category: PatternCategory::FunctionCall,
                });
            }

            // Data structure patterns
            if line.contains("Vec::") || line.contains("HashMap::") || line.contains("HashSet::") {
                motifs.push(SimplifiedMotif {
                    pattern: "collection".to_string(),
                    category: PatternCategory::DataStructure,
                });
            }

            // Common boilerplate patterns
            if line.contains("println!") || line.contains("eprintln!") || line.contains("dbg!") {
                motifs.push(SimplifiedMotif {
                    pattern: "debug_print".to_string(),
                    category: PatternCategory::Boilerplate,
                });
            }

            if line.contains("unwrap()") || line.contains("expect(") {
                motifs.push(SimplifiedMotif {
                    pattern: "error_unwrap".to_string(),
                    category: PatternCategory::Boilerplate,
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

            let category = self.categorize_motif(&motif);
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

/// Language adapter trait for AST analysis
pub trait LanguageAdapter: Send + Sync {
    fn language_name(&self) -> &str;
    fn parse_source(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<crate::lang::common::ParseIndex>;
    fn extract_ast_patterns(
        &self,
        parse_index: &crate::lang::common::ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>>;
}

/// Python language adapter implementation
pub struct PythonLanguageAdapter {
    adapter: crate::lang::python::PythonAdapter,
}

impl PythonLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::python::PythonAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for PythonLanguageAdapter {
    fn language_name(&self) -> &str {
        "python"
    }

    fn parse_source(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<crate::lang::common::ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    fn extract_ast_patterns(
        &self,
        parse_index: &crate::lang::common::ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        // Extract node type patterns from entities
        for (_id, entity) in &parse_index.entities {
            // Node type pattern
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "python".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);

            // Extract metadata-based patterns for Python-specific constructs
            if let Some(serde_json::Value::Bool(true)) = entity.metadata.get("has_decorators") {
                let decorator_pattern = AstPattern {
                    id: "decorator_usage".to_string(),
                    pattern_type: AstPatternType::FrameworkPattern,
                    node_type: None,
                    subtree_signature: Some("decorator_list".to_string()),
                    token_sequence: None,
                    language: "python".to_string(),
                    metadata: entity.metadata.clone(),
                };
                patterns.push(decorator_pattern);
            }

            // Extract function parameter patterns
            if let Some(serde_json::Value::Array(params)) = entity.metadata.get("parameters") {
                if !params.is_empty() {
                    let param_pattern = AstPattern {
                        id: format!("function_params:{}", params.len()),
                        pattern_type: AstPatternType::SubtreePattern,
                        node_type: None,
                        subtree_signature: Some(format!(
                            "function_definition->parameters[{}]",
                            params.len()
                        )),
                        token_sequence: None,
                        language: "python".to_string(),
                        metadata: HashMap::new(),
                    };
                    patterns.push(param_pattern);
                }
            }
        }

        // Extract token sequence patterns from source
        let token_patterns = self.extract_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

impl PythonLanguageAdapter {
    fn extract_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        // Common Python boilerplate patterns
        let common_sequences = vec![
            "if __name__ == \"__main__\":",
            "from typing import",
            "import os",
            "import sys",
            "def __init__(self",
            "self.",
            "return None",
            "raise ValueError",
            "except Exception",
            "with open(",
        ];

        for line in source_code.lines() {
            let line = line.trim();
            for sequence in &common_sequences {
                if line.contains(sequence) {
                    let pattern = AstPattern {
                        id: format!("token_seq:{}", sequence.replace(" ", "_")),
                        pattern_type: AstPatternType::TokenSequence,
                        node_type: None,
                        subtree_signature: None,
                        token_sequence: Some(sequence.to_string()),
                        language: "python".to_string(),
                        metadata: HashMap::new(),
                    };
                    patterns.push(pattern);
                }
            }
        }

        Ok(patterns)
    }
}

/// JavaScript language adapter implementation
pub struct JavaScriptLanguageAdapter {
    adapter: crate::lang::javascript::JavaScriptAdapter,
}

impl JavaScriptLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::javascript::JavaScriptAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for JavaScriptLanguageAdapter {
    fn language_name(&self) -> &str {
        "javascript"
    }

    fn parse_source(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<crate::lang::common::ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    fn extract_ast_patterns(
        &self,
        parse_index: &crate::lang::common::ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        // Extract entity-based patterns
        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "javascript".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        // JavaScript-specific token patterns
        let token_patterns = self.extract_js_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

impl JavaScriptLanguageAdapter {
    fn extract_js_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        let common_js_sequences = vec![
            "const ",
            "let ",
            "var ",
            "function(",
            "() => {",
            "require(",
            "module.exports",
            "console.log(",
            "JSON.stringify(",
            "JSON.parse(",
            ".then(",
            ".catch(",
            "async ",
            "await ",
        ];

        for line in source_code.lines() {
            let line = line.trim();
            for sequence in &common_js_sequences {
                if line.contains(sequence) {
                    let pattern = AstPattern {
                        id: format!(
                            "token_seq:{}",
                            sequence.replace(" ", "_").replace("(", "").replace(")", "")
                        ),
                        pattern_type: AstPatternType::TokenSequence,
                        node_type: None,
                        subtree_signature: None,
                        token_sequence: Some(sequence.to_string()),
                        language: "javascript".to_string(),
                        metadata: HashMap::new(),
                    };
                    patterns.push(pattern);
                }
            }
        }

        Ok(patterns)
    }
}

/// TypeScript language adapter implementation  
pub struct TypeScriptLanguageAdapter {
    adapter: crate::lang::typescript::TypeScriptAdapter,
}

impl TypeScriptLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::typescript::TypeScriptAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for TypeScriptLanguageAdapter {
    fn language_name(&self) -> &str {
        "typescript"
    }

    fn parse_source(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<crate::lang::common::ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    fn extract_ast_patterns(
        &self,
        parse_index: &crate::lang::common::ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        // Extract entity-based patterns
        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "typescript".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        // TypeScript-specific patterns
        let token_patterns = self.extract_ts_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

impl TypeScriptLanguageAdapter {
    fn extract_ts_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        let common_ts_sequences = vec![
            ": string",
            ": number",
            ": boolean",
            ": void",
            "interface ",
            "type ",
            "enum ",
            "export ",
            "import ",
            "extends ",
            "implements ",
            "public ",
            "private ",
            "protected ",
            "readonly ",
            "as ",
        ];

        for line in source_code.lines() {
            let line = line.trim();
            for sequence in &common_ts_sequences {
                if line.contains(sequence) {
                    let pattern = AstPattern {
                        id: format!("token_seq:{}", sequence.replace(" ", "_")),
                        pattern_type: AstPatternType::TokenSequence,
                        node_type: None,
                        subtree_signature: None,
                        token_sequence: Some(sequence.to_string()),
                        language: "typescript".to_string(),
                        metadata: HashMap::new(),
                    };
                    patterns.push(pattern);
                }
            }
        }

        Ok(patterns)
    }
}

/// Rust language adapter implementation
pub struct RustLanguageAdapter {
    adapter: crate::lang::rust_lang::RustAdapter,
}

impl RustLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::rust_lang::RustAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for RustLanguageAdapter {
    fn language_name(&self) -> &str {
        "rust"
    }

    fn parse_source(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<crate::lang::common::ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    fn extract_ast_patterns(
        &self,
        parse_index: &crate::lang::common::ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "rust".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        let token_patterns = self.extract_rust_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

impl RustLanguageAdapter {
    fn extract_rust_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        let common_rust_sequences = vec![
            "use ",
            "pub ",
            "fn ",
            "struct ",
            "enum ",
            "impl ",
            "trait ",
            "let ",
            "mut ",
            "&self",
            "&mut self",
            "Result<",
            "Option<",
            "Vec<",
            "HashMap<",
            "println!",
            "eprintln!",
            "dbg!",
            ".unwrap()",
            ".expect(",
            "match ",
            "if let",
            "Some(",
            "None",
            "Ok(",
            "Err(",
        ];

        for line in source_code.lines() {
            let line = line.trim();
            for sequence in &common_rust_sequences {
                if line.contains(sequence) {
                    let pattern = AstPattern {
                        id: format!(
                            "token_seq:{}",
                            sequence.replace(" ", "_").replace("<", "").replace("(", "")
                        ),
                        pattern_type: AstPatternType::TokenSequence,
                        node_type: None,
                        subtree_signature: None,
                        token_sequence: Some(sequence.to_string()),
                        language: "rust".to_string(),
                        metadata: HashMap::new(),
                    };
                    patterns.push(pattern);
                }
            }
        }

        Ok(patterns)
    }
}

/// Go language adapter implementation
pub struct GoLanguageAdapter {
    adapter: crate::lang::go::GoAdapter,
}

impl GoLanguageAdapter {
    pub fn new() -> Result<Self> {
        let adapter = crate::lang::go::GoAdapter::new()?;
        Ok(Self { adapter })
    }
}

impl LanguageAdapter for GoLanguageAdapter {
    fn language_name(&self) -> &str {
        "go"
    }

    fn parse_source(
        &mut self,
        source_code: &str,
        file_path: &str,
    ) -> Result<crate::lang::common::ParseIndex> {
        self.adapter.parse_source(source_code, file_path)
    }

    fn extract_ast_patterns(
        &self,
        parse_index: &crate::lang::common::ParseIndex,
        source_code: &str,
    ) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        for (_id, entity) in &parse_index.entities {
            let node_type = format!("{:?}", entity.kind);
            let node_pattern = AstPattern {
                id: format!("node_type:{}", node_type),
                pattern_type: AstPatternType::NodeType,
                node_type: Some(node_type),
                subtree_signature: None,
                token_sequence: None,
                language: "go".to_string(),
                metadata: HashMap::new(),
            };
            patterns.push(node_pattern);
        }

        let token_patterns = self.extract_go_token_sequences(source_code)?;
        patterns.extend(token_patterns);

        Ok(patterns)
    }
}

impl GoLanguageAdapter {
    fn extract_go_token_sequences(&self, source_code: &str) -> Result<Vec<AstPattern>> {
        let mut patterns = Vec::new();

        let common_go_sequences = vec![
            "package ",
            "import ",
            "func ",
            "var ",
            "const ",
            "type ",
            "struct {",
            "interface {",
            "if err != nil",
            "return ",
            "fmt.Println(",
            "fmt.Printf(",
            "log.Fatal(",
            "make(",
            "append(",
            "len(",
            "cap(",
            ":= ",
            "go ",
            "defer ",
            "chan ",
            "select {",
            "for ",
            "range ",
        ];

        for line in source_code.lines() {
            let line = line.trim();
            for sequence in &common_go_sequences {
                if line.contains(sequence) {
                    let pattern = AstPattern {
                        id: format!(
                            "token_seq:{}",
                            sequence.replace(" ", "_").replace("{", "").replace("(", "")
                        ),
                        pattern_type: AstPatternType::TokenSequence,
                        node_type: None,
                        subtree_signature: None,
                        token_sequence: Some(sequence.to_string()),
                        language: "go".to_string(),
                        metadata: HashMap::new(),
                    };
                    patterns.push(pattern);
                }
            }
        }

        Ok(patterns)
    }
}

/// AST pattern extracted from tree-sitter analysis
#[derive(Debug, Clone)]
pub struct AstPattern {
    /// Pattern identifier
    pub id: String,

    /// Pattern type
    pub pattern_type: AstPatternType,

    /// Node type (for NodeType patterns)
    pub node_type: Option<String>,

    /// Subtree structure (for SubtreePattern)
    pub subtree_signature: Option<String>,

    /// Token sequence (for TokenSequence patterns)
    pub token_sequence: Option<String>,

    /// Language where pattern was found
    pub language: String,

    /// Metadata about the pattern
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Types of AST patterns that can be extracted
#[derive(Debug, Clone, PartialEq)]
pub enum AstPatternType {
    /// Common AST node type
    NodeType,

    /// Structural subtree pattern
    SubtreePattern,

    /// Token sequence pattern
    TokenSequence,

    /// Control flow pattern
    ControlFlowPattern,

    /// Framework-specific pattern
    FrameworkPattern,
}

/// AST pattern extractor that analyzes parsed code
#[derive(Debug)]
pub struct AstPatternExtractor {
    /// Node type frequency tracking
    node_type_frequencies: HashMap<String, usize>,

    /// Subtree pattern frequencies
    subtree_frequencies: HashMap<String, usize>,

    /// Token sequence frequencies
    token_sequence_frequencies: HashMap<String, usize>,

    /// Pattern extraction configuration
    config: AstExtractionConfig,
}

/// Configuration for AST pattern extraction
#[derive(Debug, Clone)]
pub struct AstExtractionConfig {
    /// Minimum support count for patterns
    pub min_support: usize,

    /// Maximum subtree depth to analyze
    pub max_subtree_depth: usize,

    /// Token sequence length for analysis
    pub token_sequence_length: usize,

    /// Languages to process
    pub enabled_languages: HashSet<String>,
}

/// Frequency thresholds for pattern selection
#[derive(Debug, Clone)]
pub struct PatternThresholds {
    /// Top percentile for node types (e.g., top 5%)
    pub node_type_percentile: f64,

    /// Top percentile for subtree patterns
    pub subtree_percentile: f64,

    /// Top percentile for token sequences
    pub token_sequence_percentile: f64,

    /// Minimum IDF score for pattern selection
    pub min_idf_score: f64,
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

        let config = AstExtractionConfig {
            min_support: 3,
            max_subtree_depth: 4,
            token_sequence_length: 5,
            enabled_languages: ["python", "javascript", "typescript", "rust", "go"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        };

        let thresholds = PatternThresholds {
            node_type_percentile: 0.95,      // Top 5% most frequent node types
            subtree_percentile: 0.90,        // Top 10% most frequent subtrees
            token_sequence_percentile: 0.95, // Top 5% most frequent token sequences
            min_idf_score: 0.1,
        };

        Self {
            language_adapters,
            pattern_extractor: AstPatternExtractor::new(config.clone()),
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

impl AstPatternExtractor {
    /// Create a new AST pattern extractor
    pub fn new(config: AstExtractionConfig) -> Self {
        Self {
            node_type_frequencies: HashMap::new(),
            subtree_frequencies: HashMap::new(),
            token_sequence_frequencies: HashMap::new(),
            config,
        }
    }

    /// Analyze frequencies of all extracted patterns
    pub fn analyze_pattern_frequencies(&mut self, patterns: &[AstPattern]) {
        self.node_type_frequencies.clear();
        self.subtree_frequencies.clear();
        self.token_sequence_frequencies.clear();

        for pattern in patterns {
            match &pattern.pattern_type {
                AstPatternType::NodeType => {
                    if let Some(ref node_type) = pattern.node_type {
                        *self
                            .node_type_frequencies
                            .entry(node_type.clone())
                            .or_insert(0) += 1;
                    }
                }
                AstPatternType::SubtreePattern => {
                    if let Some(ref signature) = pattern.subtree_signature {
                        *self
                            .subtree_frequencies
                            .entry(signature.clone())
                            .or_insert(0) += 1;
                    }
                }
                AstPatternType::TokenSequence => {
                    if let Some(ref sequence) = pattern.token_sequence {
                        *self
                            .token_sequence_frequencies
                            .entry(sequence.clone())
                            .or_insert(0) += 1;
                    }
                }
                AstPatternType::ControlFlowPattern => {
                    // Treat as subtree pattern for frequency analysis
                    if let Some(ref signature) = pattern.subtree_signature {
                        *self
                            .subtree_frequencies
                            .entry(signature.clone())
                            .or_insert(0) += 1;
                    }
                }
                AstPatternType::FrameworkPattern => {
                    // Treat as subtree pattern for frequency analysis
                    if let Some(ref signature) = pattern.subtree_signature {
                        *self
                            .subtree_frequencies
                            .entry(signature.clone())
                            .or_insert(0) += 1;
                    }
                }
            }
        }
    }
}

impl Default for AstExtractionConfig {
    fn default() -> Self {
        Self {
            min_support: 3,
            max_subtree_depth: 4,
            token_sequence_length: 5,
            enabled_languages: ["python", "javascript", "typescript", "rust", "go"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

impl Default for PatternThresholds {
    fn default() -> Self {
        Self {
            node_type_percentile: 0.95,
            subtree_percentile: 0.90,
            token_sequence_percentile: 0.95,
            min_idf_score: 0.1,
        }
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
mod tests {
    use super::*;
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::{tempdir, TempDir};

    fn sample_codebase_info() -> CodebaseInfo {
        let mut file_info = HashMap::new();
        let hash = Sha256::digest(b"fn sample() {}").to_vec();
        file_info.insert(
            "sample.rs".to_string(),
            FileInfo {
                line_count: 2,
                content_hash: hash,
            },
        );

        CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "sample".to_string(),
                source_code: "fn sample() {\n    let value = 42;\n}".to_string(),
                file_path: "sample.rs".to_string(),
                line_count: 2,
            }],
            total_lines: 2,
            file_info,
        }
    }

    fn write_cache(manager: &StopMotifCacheManager, cache: &StopMotifCache) {
        let cache_path = manager.get_cache_path();
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let serialized = serde_json::to_string_pretty(cache).unwrap();
        fs::write(cache_path, serialized).unwrap();
    }

    #[test]
    fn test_get_valid_cache_returns_none_when_expired() {
        let temp_dir = TempDir::new().unwrap();
        let mut policy = CacheRefreshPolicy::default();
        policy.max_age_days = 1;
        let manager = StopMotifCacheManager::new(temp_dir.path(), policy.clone());

        let codebase = sample_codebase_info();
        let signature = manager.compute_codebase_signature(&codebase);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let expired_cache = StopMotifCache {
            version: 1,
            k_gram_size: policy.k_gram_size,
            token_grams: Vec::new(),
            pdg_motifs: Vec::new(),
            ast_patterns: Vec::new(),
            last_updated: now - (policy.max_age_days * 24 * 60 * 60) - 1,
            codebase_signature: signature,
            mining_stats: MiningStats::default(),
        };

        write_cache(&manager, &expired_cache);

        let result = manager.get_valid_cache(&codebase).unwrap();
        assert!(result.is_none(), "expected expired cache to be invalidated");
    }

    #[test]
    fn test_get_valid_cache_returns_none_on_large_signature_change() {
        let temp_dir = TempDir::new().unwrap();
        let mut policy = CacheRefreshPolicy::default();
        policy.change_threshold_percent = 1.0;
        let manager = StopMotifCacheManager::new(temp_dir.path(), policy.clone());

        let original = sample_codebase_info();
        let signature = manager.compute_codebase_signature(&original);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache = StopMotifCache {
            version: 1,
            k_gram_size: policy.k_gram_size,
            token_grams: Vec::new(),
            pdg_motifs: Vec::new(),
            ast_patterns: Vec::new(),
            last_updated: now,
            codebase_signature: signature,
            mining_stats: MiningStats::default(),
        };

        write_cache(&manager, &cache);

        let mut updated = sample_codebase_info();
        updated.total_lines = 10;

        let result = manager.get_valid_cache(&updated).unwrap();
        assert!(
            result.is_none(),
            "expected cache to be refreshed when signature diverges"
        );
    }

    #[test]
    fn test_get_valid_cache_returns_cache_when_fresh() {
        let temp_dir = TempDir::new().unwrap();
        let policy = CacheRefreshPolicy::default();
        let manager = StopMotifCacheManager::new(temp_dir.path(), policy.clone());

        let codebase = sample_codebase_info();
        let signature = manager.compute_codebase_signature(&codebase);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let cache = StopMotifCache {
            version: 1,
            k_gram_size: policy.k_gram_size,
            token_grams: Vec::new(),
            pdg_motifs: Vec::new(),
            ast_patterns: Vec::new(),
            last_updated: now,
            codebase_signature: signature,
            mining_stats: MiningStats::default(),
        };

        write_cache(&manager, &cache);

        let result = manager.get_valid_cache(&codebase).unwrap();
        assert!(result.is_some(), "expected fresh cache to remain valid");
    }

    #[test]
    fn test_pattern_miner_extracts_kgrams_and_motifs() {
        let mut policy = CacheRefreshPolicy::default();
        policy.k_gram_size = 2;
        let miner = PatternMiner::new(policy);

        let function = FunctionInfo {
            id: "f".to_string(),
            source_code: "if value == 10 {\n    println!(\"value\");\n    total += value;\n}"
                .to_string(),
            file_path: "sample.rs".to_string(),
            line_count: 4,
        };

        let kgrams = miner.extract_function_kgrams(&function);
        assert!(
            !kgrams.is_empty(),
            "expected k-grams when token window threshold is satisfied"
        );

        let motifs = miner.extract_function_motifs(&function).unwrap();
        assert!(
            motifs.keys().any(|key| key.contains("control")),
            "expected control flow motif"
        );
        assert!(
            motifs.keys().any(|key| key.contains("boiler")),
            "expected boilerplate motif from println!/unwrap"
        );
    }

    #[test]
    fn test_pattern_miner_select_stop_motifs_respects_percentile() {
        let mut policy = CacheRefreshPolicy::default();
        policy.stop_motif_percentile = 1.0;
        let mut miner = PatternMiner::new(policy);

        miner.kgram_frequencies = HashMap::from([
            ("alpha beta".to_string(), 10),
            ("beta gamma".to_string(), 5),
        ]);
        miner.motif_frequencies = HashMap::from([("call:helper".to_string(), 7)]);
        miner.total_documents = 20;

        let idf_scores = miner.calculate_idf_scores();
        let stop_motifs = miner.select_stop_motifs(&idf_scores).unwrap();
        assert_eq!(stop_motifs.len(), 1, "percentile should cap the selection");
        assert_eq!(stop_motifs[0].pattern, "alpha beta");
        assert_eq!(
            stop_motifs[0].category,
            PatternCategory::TokenGram,
            "expected token gram category for highest frequency k-gram"
        );
    }

    #[test]
    fn test_normalize_token_handles_literals_and_keywords() {
        let mut policy = CacheRefreshPolicy::default();
        policy.k_gram_size = 2;
        let miner = PatternMiner::new(policy);

        assert_eq!(miner.normalize_token("if"), "if");
        assert_eq!(miner.normalize_token("=="), "==");
        assert_eq!(miner.normalize_token("42"), "INT_LIT");
        assert_eq!(miner.normalize_token("3.14"), "FLOAT_LIT");
        assert_eq!(miner.normalize_token("\"text\""), "STR_LIT");
        assert_eq!(miner.normalize_token("variable_name"), "LOCAL_VAR");
        assert_eq!(miner.normalize_token("SOME_CONSTANT"), "SOME_CONSTANT");
    }

    #[test]
    fn test_compute_codebase_signature_deterministic() {
        let policy = CacheRefreshPolicy::default();
        let manager = StopMotifCacheManager::new("unused", policy);
        let info = sample_codebase_info();
        let sig1 = manager.compute_codebase_signature(&info);
        let sig2 = manager.compute_codebase_signature(&info);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_estimate_change_percentage_detects_difference() {
        let policy = CacheRefreshPolicy::default();
        let manager = StopMotifCacheManager::new("unused", policy);
        assert_eq!(manager.estimate_change_percentage("aaaa", "aaaa"), 0.0);
        assert!(
            manager.estimate_change_percentage("aaaa", "bbbb") >= 50.0,
            "expected large heuristic change"
        );
    }

    #[test]
    fn test_ast_stop_motif_miner_extracts_patterns() -> Result<()> {
        let mut miner = AstStopMotifMiner::new();
        let functions = vec![
            FunctionInfo {
                id: "py_func".to_string(),
                source_code: "def greet(name):\n    print(f\"hi {name}\")\n".to_string(),
                file_path: "greet.py".to_string(),
                line_count: 2,
            },
            FunctionInfo {
                id: "js_func".to_string(),
                source_code: "export function add(a, b) { return a + b; }\n".to_string(),
                file_path: "math.js".to_string(),
                line_count: 1,
            },
        ];

        let patterns = miner.mine_ast_stop_motifs(&functions)?;
        assert!(
            patterns.len() <= functions.len(),
            "stop-motif selection should not exceed number of functions"
        );

        Ok(())
    }

    #[test]
    fn test_stop_motif_cache_serialization() {
        let cache = StopMotifCache {
            version: 1,
            k_gram_size: 9,
            token_grams: vec![
                StopMotifEntry {
                    pattern: "if LOCAL_VAR == INT_LIT".to_string(),
                    support: 150,
                    idf_score: 2.5,
                    weight_multiplier: 0.2,
                    category: PatternCategory::TokenGram,
                },
                StopMotifEntry {
                    pattern: "println! ( STR_LIT )".to_string(),
                    support: 89,
                    idf_score: 1.8,
                    weight_multiplier: 0.2,
                    category: PatternCategory::TokenGram,
                },
            ],
            pdg_motifs: vec![
                StopMotifEntry {
                    pattern: "control:branch".to_string(),
                    support: 200,
                    idf_score: 3.2,
                    weight_multiplier: 0.2,
                    category: PatternCategory::ControlFlow,
                },
                StopMotifEntry {
                    pattern: "boiler:debug_print".to_string(),
                    support: 95,
                    idf_score: 1.9,
                    weight_multiplier: 0.2,
                    category: PatternCategory::Boilerplate,
                },
            ],
            ast_patterns: vec![
                AstStopMotifEntry {
                    pattern: "node_type:Function".to_string(),
                    support: 300,
                    idf_score: 2.1,
                    weight_multiplier: 0.2,
                    category: AstPatternCategory::NodeType,
                    language: "python".to_string(),
                    metadata: HashMap::new(),
                },
                AstStopMotifEntry {
                    pattern: "token_seq:import_os".to_string(),
                    support: 120,
                    idf_score: 1.8,
                    weight_multiplier: 0.2,
                    category: AstPatternCategory::TokenSequence,
                    language: "python".to_string(),
                    metadata: HashMap::new(),
                },
            ],
            last_updated: 1699123456,
            codebase_signature: "abc123def456".to_string(),
            mining_stats: MiningStats {
                functions_analyzed: 1500,
                unique_kgrams_found: 8000,
                unique_motifs_found: 1200,
                ast_patterns_found: 2,
                ast_node_types_found: 1,
                ast_subtree_patterns_found: 0,
                stop_motifs_selected: 6, // Updated to include AST patterns
                percentile_threshold: 0.5,
                mining_duration_ms: 2500,
                languages_processed: ["python".to_string(), "rust".to_string()]
                    .into_iter()
                    .collect(),
            },
        };

        // Test serialization
        let json = serde_json::to_string_pretty(&cache).expect("Failed to serialize cache");
        assert!(json.contains("\"version\": 1"));
        assert!(json.contains("\"k_gram_size\": 9"));
        assert!(json.contains("if LOCAL_VAR == INT_LIT"));
        assert!(json.contains("control:branch"));

        // Test deserialization
        let deserialized: StopMotifCache =
            serde_json::from_str(&json).expect("Failed to deserialize cache");
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.token_grams.len(), 2);
        assert_eq!(deserialized.pdg_motifs.len(), 2);
        assert_eq!(deserialized.mining_stats.functions_analyzed, 1500);
    }

    #[test]
    fn test_pattern_miner_kgram_extraction() {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        let func = FunctionInfo {
            id: "test_func".to_string(),
            source_code: r#"
                fn test_function() {
                    if x == 42 {
                        println!("Hello world");
                    }
                    for i in 0..10 {
                        process_item(i);
                    }
                }
            "#
            .to_string(),
            file_path: "test.rs".to_string(),
            line_count: 8,
        };

        let kgrams = miner.extract_function_kgrams(&func);

        // Should have various k-grams including normalized patterns
        assert!(!kgrams.is_empty());

        // Check that normalization occurred
        let has_normalized = kgrams
            .keys()
            .any(|k| k.contains("LOCAL_VAR") || k.contains("INT_LIT") || k.contains("STR_LIT"));
        assert!(has_normalized, "Should contain normalized tokens");

        // Check for control flow patterns
        let has_control_flow = kgrams.keys().any(|k| k.contains("if") || k.contains("for"));
        assert!(has_control_flow, "Should contain control flow patterns");
    }

    #[test]
    fn test_pattern_miner_motif_extraction() -> Result<()> {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        let func = FunctionInfo {
            id: "test_func".to_string(),
            source_code: r#"
                fn complex_function() {
                    if condition {
                        println!("debug message");
                    }
                    for item in items {
                        let result = process(item).unwrap();
                        data.push(result);
                    }
                    while active {
                        update_state();
                    }
                }
            "#
            .to_string(),
            file_path: "test.rs".to_string(),
            line_count: 12,
        };

        let motifs = miner.extract_function_motifs(&func)?;

        // Should extract various motif types
        assert!(!motifs.is_empty());

        // Check for expected patterns
        let motif_keys: Vec<_> = motifs.keys().collect();
        let has_control = motif_keys
            .iter()
            .any(|k| k.contains("control:branch") || k.contains("control:loop"));
        let has_boilerplate = motif_keys
            .iter()
            .any(|k| k.contains("boiler:debug_print") || k.contains("boiler:error_unwrap"));
        let has_assignment = motif_keys.iter().any(|k| k.contains("assign:assign"));
        let has_calls = motif_keys.iter().any(|k| k.contains("call:call"));

        assert!(has_control, "Should extract control flow motifs");
        assert!(has_boilerplate, "Should extract boilerplate motifs");
        assert!(has_assignment, "Should extract assignment motifs");
        assert!(has_calls, "Should extract function call motifs");

        Ok(())
    }

    #[test]
    fn test_pattern_miner_stop_motif_selection() -> Result<()> {
        let policy = CacheRefreshPolicy {
            stop_motif_percentile: 50.0, // Top 50% for easier testing
            ..Default::default()
        };
        let mut miner = PatternMiner::new(policy);

        let codebase_info = CodebaseInfo {
            functions: vec![
                FunctionInfo {
                    id: "func1".to_string(),
                    source_code: "fn func1() { println!(\"test\"); }".to_string(),
                    file_path: "file1.rs".to_string(),
                    line_count: 1,
                },
                FunctionInfo {
                    id: "func2".to_string(),
                    source_code: "fn func2() { println!(\"test2\"); if x > 0 { process(); } }"
                        .to_string(),
                    file_path: "file2.rs".to_string(),
                    line_count: 1,
                },
                FunctionInfo {
                    id: "func3".to_string(),
                    source_code: "fn func3() { if condition { println!(\"debug\"); } }".to_string(),
                    file_path: "file3.rs".to_string(),
                    line_count: 1,
                },
            ],
            total_lines: 3,
            file_info: HashMap::new(),
        };

        let cache = miner.mine_stop_motifs(&codebase_info)?;

        // Verify cache structure
        assert_eq!(cache.version, 1);
        assert_eq!(cache.mining_stats.functions_analyzed, 3);
        assert!(cache.mining_stats.stop_motifs_selected > 0);

        // Should have both token grams and motifs
        assert!(!cache.token_grams.is_empty() || !cache.pdg_motifs.is_empty());

        // All stop motifs should have weight multiplier of 0.2
        for stop_motif in &cache.token_grams {
            assert_eq!(stop_motif.weight_multiplier, 0.2);
            assert!(stop_motif.support > 0);
        }

        for stop_motif in &cache.pdg_motifs {
            assert_eq!(stop_motif.weight_multiplier, 0.2);
            assert!(stop_motif.support > 0);
        }

        Ok(())
    }

    #[test]
    fn test_cache_manager_persistence() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy::default();
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let codebase_info = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "test_func".to_string(),
                source_code: "fn test() { println!(\"test\"); }".to_string(),
                file_path: "test.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 1,
            file_info: HashMap::new(),
        };

        // First call should create cache
        let cache1 = cache_manager.get_cache(&codebase_info)?;
        assert_eq!(cache1.mining_stats.functions_analyzed, 1);

        // Verify cache file was created
        let cache_path = cache_dir.join("stop_motifs.v1.json");
        assert!(cache_path.exists());

        // Second call should load from cache (same codebase signature)
        let cache2 = cache_manager.get_cache(&codebase_info)?;
        assert_eq!(cache2.mining_stats.functions_analyzed, 1);
        assert_eq!(cache1.codebase_signature, cache2.codebase_signature);

        Ok(())
    }

    #[test]
    fn test_cache_invalidation_by_change() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy {
            change_threshold_percent: 1.0, // Very low threshold for testing
            ..Default::default()
        };
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let codebase_info1 = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "func1".to_string(),
                source_code: "fn func1() { println!(\"test\"); }".to_string(),
                file_path: "test.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 1,
            file_info: HashMap::new(),
        };

        let codebase_info2 = CodebaseInfo {
            functions: vec![
                FunctionInfo {
                    id: "func1".to_string(),
                    source_code: "fn func1() { println!(\"test\"); }".to_string(),
                    file_path: "test.rs".to_string(),
                    line_count: 1,
                },
                FunctionInfo {
                    id: "func2".to_string(),
                    source_code: "fn func2() { if x > 0 { process(); } }".to_string(),
                    file_path: "test2.rs".to_string(),
                    line_count: 1,
                },
            ],
            total_lines: 2,
            file_info: HashMap::new(),
        };

        // Create initial cache
        let cache1 = cache_manager.get_cache(&codebase_info1)?;
        let sig1 = cache1.codebase_signature.clone();

        // Changed codebase should trigger refresh
        let cache2 = cache_manager.get_cache(&codebase_info2)?;
        let sig2 = cache2.codebase_signature.clone();

        assert_ne!(
            sig1, sig2,
            "Signatures should differ for different codebases"
        );
        assert_eq!(cache2.mining_stats.functions_analyzed, 2);

        Ok(())
    }

    #[test]
    fn test_cache_retains_when_change_below_threshold() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy {
            change_threshold_percent: 75.0,
            ..Default::default()
        };
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let mut base_file_info = HashMap::new();
        base_file_info.insert(
            "src/lib.rs".to_string(),
            FileInfo {
                line_count: 10,
                content_hash: vec![1, 2, 3, 4],
            },
        );

        let base_info = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "func1".to_string(),
                source_code: "fn func1() {}".to_string(),
                file_path: "src/lib.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 10,
            file_info: base_file_info.clone(),
        };

        let cache1 = cache_manager.get_cache(&base_info)?;
        assert_eq!(cache1.mining_stats.functions_analyzed, 1);

        let mut changed_info = base_info.clone();
        changed_info.functions.push(FunctionInfo {
            id: "func2".to_string(),
            source_code: "fn func2() {}".to_string(),
            file_path: "src/new.rs".to_string(),
            line_count: 1,
        });
        changed_info.total_lines = 11;
        let mut changed_file_info = base_file_info;
        changed_file_info.insert(
            "src/new.rs".to_string(),
            FileInfo {
                line_count: 5,
                content_hash: vec![9, 9, 9, 9],
            },
        );
        changed_info.file_info = changed_file_info;

        let cache2 = cache_manager.get_cache(&changed_info)?;
        assert_eq!(
            cache2.codebase_signature, cache1.codebase_signature,
            "expected cache reuse when change below threshold"
        );
        assert_eq!(
            cache2.mining_stats.functions_analyzed, 1,
            "expected mining stats unchanged for reused cache"
        );

        Ok(())
    }

    #[test]
    fn test_cache_expires_when_past_max_age() -> Result<()> {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_dir = temp_dir.path().to_path_buf();

        let policy = CacheRefreshPolicy {
            max_age_days: 0,
            ..Default::default()
        };
        let cache_manager = StopMotifCacheManager::new(&cache_dir, policy);

        let codebase_info = CodebaseInfo {
            functions: vec![FunctionInfo {
                id: "func1".to_string(),
                source_code: "fn func1() {}".to_string(),
                file_path: "src/lib.rs".to_string(),
                line_count: 1,
            }],
            total_lines: 1,
            file_info: HashMap::new(),
        };

        let cache1 = cache_manager.get_cache(&codebase_info)?;
        let cache_path = cache_dir.join("stop_motifs.v1.json");
        assert!(cache_path.exists());

        let mut cache_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&cache_path)?).expect("parse cache json");
        if let Some(obj) = cache_json.as_object_mut() {
            obj.insert("last_updated".to_string(), json!(0));
        }
        fs::write(&cache_path, serde_json::to_string_pretty(&cache_json)?)?;

        let refreshed = cache_manager.get_cache(&codebase_info)?;
        let refreshed_file: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&cache_path)?)
                .expect("parse refreshed cache json");
        let refreshed_disk = refreshed_file["last_updated"]
            .as_u64()
            .expect("last_updated should be number");
        assert_eq!(refreshed_disk, refreshed.last_updated);
        assert!(refreshed.last_updated >= cache1.last_updated);
        assert!(
            refreshed.last_updated > 0,
            "expected refreshed cache timestamp to be non-zero"
        );

        Ok(())
    }

    #[test]
    fn test_compute_codebase_signature_order_independent() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let cache_manager =
            StopMotifCacheManager::new(temp_dir.path(), CacheRefreshPolicy::default());

        let mut file_info_a = HashMap::new();
        file_info_a.insert(
            "b.rs".to_string(),
            FileInfo {
                line_count: 20,
                content_hash: vec![2, 3, 4],
            },
        );
        file_info_a.insert(
            "a.rs".to_string(),
            FileInfo {
                line_count: 10,
                content_hash: vec![1, 2, 3],
            },
        );
        let info_a = CodebaseInfo {
            functions: vec![],
            total_lines: 30,
            file_info: file_info_a,
        };

        let mut file_info_b = HashMap::new();
        file_info_b.insert(
            "a.rs".to_string(),
            FileInfo {
                line_count: 10,
                content_hash: vec![1, 2, 3],
            },
        );
        file_info_b.insert(
            "b.rs".to_string(),
            FileInfo {
                line_count: 20,
                content_hash: vec![2, 3, 4],
            },
        );
        let info_b = CodebaseInfo {
            functions: vec![],
            total_lines: 30,
            file_info: file_info_b,
        };

        let sig_a = cache_manager.compute_codebase_signature(&info_a);
        let sig_b = cache_manager.compute_codebase_signature(&info_b);
        assert_eq!(
            sig_a, sig_b,
            "expected signature independence from file ordering"
        );
    }

    #[test]
    fn test_pattern_normalization() {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        // Test token normalization
        assert_eq!(miner.normalize_token("42"), "INT_LIT");
        assert_eq!(miner.normalize_token("3.14"), "FLOAT_LIT");
        assert_eq!(miner.normalize_token("\"hello\""), "STR_LIT");
        assert_eq!(miner.normalize_token("'c'"), "STR_LIT");
        assert_eq!(miner.normalize_token("local_var"), "LOCAL_VAR");
        assert_eq!(miner.normalize_token("CONSTANT"), "CONSTANT");
        assert_eq!(miner.normalize_token("function_name"), "LOCAL_VAR");
    }

    #[test]
    fn test_motif_categorization() {
        let policy = CacheRefreshPolicy::default();
        let miner = PatternMiner::new(policy);

        // Test motif categorization
        assert_eq!(
            miner.categorize_motif("control:branch"),
            PatternCategory::ControlFlow
        );
        assert_eq!(
            miner.categorize_motif("control:loop"),
            PatternCategory::ControlFlow
        );
        assert_eq!(
            miner.categorize_motif("assign:assign"),
            PatternCategory::Assignment
        );
        assert_eq!(
            miner.categorize_motif("call:call"),
            PatternCategory::FunctionCall
        );
        assert_eq!(
            miner.categorize_motif("data:collection"),
            PatternCategory::DataStructure
        );
        assert_eq!(
            miner.categorize_motif("boiler:debug_print"),
            PatternCategory::Boilerplate
        );
        assert_eq!(
            miner.categorize_motif("boiler:error_unwrap"),
            PatternCategory::Boilerplate
        );
        assert_eq!(
            miner.categorize_motif("unknown:pattern"),
            PatternCategory::Boilerplate
        );
    }

    #[test]
    fn test_cache_new() {
        let cache = Cache::new();
        // Basic test to ensure new() works
        assert_eq!(std::mem::size_of_val(&cache), std::mem::size_of::<Cache>());
    }

    #[test]
    fn test_cache_default() {
        let cache = Cache::default();
        // Basic test to ensure default() works
        assert_eq!(std::mem::size_of_val(&cache), std::mem::size_of::<Cache>());
    }

    #[test]
    fn test_cache_debug() {
        let cache = Cache::new();
        let debug_str = format!("{:?}", cache);
        assert_eq!(debug_str, "Cache");
    }
}
