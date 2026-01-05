//! Pattern Mining Engine for extracting frequent k-grams and PDG motifs.
//!
//! This module provides the `PatternMiner` for mining stop-motifs from codebases.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use sha2::{Digest, Sha256};

use super::{
    AstPatternCategory, AstStopMotifEntry, AstStopMotifMiner, CacheRefreshPolicy, CodebaseInfo,
    FunctionInfo, MiningStats, PatternCategory, StopMotifCache, StopMotifEntry,
};
use crate::core::errors::Result;

/// Pattern Mining Engine for extracting frequent k-grams and PDG motifs
#[derive(Debug)]
pub struct PatternMiner {
    /// Refresh policy with mining parameters
    policy: CacheRefreshPolicy,

    /// K-gram frequency map
    pub(crate) kgram_frequencies: HashMap<String, usize>,

    /// PDG motif frequency map
    pub(crate) motif_frequencies: HashMap<String, usize>,

    /// Total documents (functions) processed
    pub(crate) total_documents: usize,
}

/// Factory and stop-motif mining methods for [`PatternMiner`].
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
    pub(crate) fn extract_function_kgrams(&self, func: &FunctionInfo) -> HashMap<String, usize> {
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
    pub(crate) fn extract_function_motifs(&self, func: &FunctionInfo) -> Result<HashMap<String, usize>> {
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
    pub(crate) fn calculate_idf_scores(&self) -> HashMap<String, f64> {
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
    pub(crate) fn select_stop_motifs(&self, idf_scores: &HashMap<String, f64>) -> Result<Vec<StopMotifEntry>> {
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
    pub(crate) fn normalize_token(&self, token: &str) -> String {
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
    pub(crate) fn categorize_motif(&self, motif: &str) -> PatternCategory {
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

/// Category string conversion methods for [`SimplifiedMotif`].
impl SimplifiedMotif {
    /// Returns the short string identifier for this motif's category.
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
