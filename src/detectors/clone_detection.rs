//! Comprehensive Data-Driven Clone Detection System
//!
//! This module implements a sophisticated clone detection system with:
//! - TF-IDF weighted structure analysis
//! - Language-agnostic normalization
//! - PDG motif analysis with WL-hashing
//! - Weighted MinHash/LSH for similarity
//! - Self-learning boilerplate detection
//! - Adaptive ranking and auto-calibration

use std::collections::{HashMap, HashSet, BTreeMap};
use std::sync::Arc;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use rayon::prelude::*;
use ahash::AHasher;
use std::hash::{Hash, Hasher};
// Removed lazy_static import as it's no longer needed after regex removal

use crate::core::featureset::{FeatureExtractor, FeatureDefinition, CodeEntity, ExtractionContext};
use crate::core::errors::{Result, ValknutError};
use crate::core::config::DedupeConfig;
use crate::io::cache::{StopMotifCacheManager, CodebaseInfo, FunctionInfo, FileInfo, CacheRefreshPolicy};
use sha2::Digest;

/// TF-IDF Analysis Engine for structure-aware clone detection
#[derive(Debug)]
pub struct TfIdfAnalyzer {
    /// Term frequencies: document_id -> term -> frequency
    term_frequencies: HashMap<String, HashMap<String, f64>>,
    
    /// Document frequencies: term -> number of documents containing term
    document_frequencies: HashMap<String, usize>,
    
    /// Total number of documents processed
    total_documents: usize,
    
    /// IDF scores cache: term -> IDF score
    idf_cache: HashMap<String, f64>,
    
    /// Language-specific normalization settings
    normalization_config: NormalizationConfig,
    
    /// Phase 3: Stop-motifs cache for automatic boilerplate filtering
    stop_motif_cache: Option<Arc<crate::io::cache::StopMotifCache>>,
}

impl TfIdfAnalyzer {
    /// Create a new TF-IDF analyzer
    pub fn new(normalization_config: NormalizationConfig) -> Self {
        Self {
            term_frequencies: HashMap::new(),
            document_frequencies: HashMap::new(),
            total_documents: 0,
            idf_cache: HashMap::new(),
            normalization_config,
            stop_motif_cache: None,
        }
    }
    
    /// Set the stop-motifs cache for Phase 3 filtering
    pub fn set_stop_motif_cache(&mut self, cache: Arc<crate::io::cache::StopMotifCache>) {
        let token_grams_len = cache.token_grams.len();
        let pdg_motifs_len = cache.pdg_motifs.len();
        self.stop_motif_cache = Some(cache);
        tracing::info!("Phase 3 stop-motifs cache enabled: {} token grams, {} PDG motifs", 
                      token_grams_len, pdg_motifs_len);
    }
    
    /// Add a document to the corpus for analysis
    pub fn add_document(&mut self, doc_id: String, tokens: Vec<String>) {
        let mut tf_map = HashMap::new();
        let mut unique_terms = HashSet::new();
        
        // Calculate term frequencies
        for token in tokens {
            let normalized_token = self.normalize_token(&token);
            *tf_map.entry(normalized_token.clone()).or_insert(0.0) += 1.0;
            unique_terms.insert(normalized_token);
        }
        
        // Normalize TF by document length
        let doc_length = tf_map.values().sum::<f64>();
        if doc_length > 0.0 {
            for tf in tf_map.values_mut() {
                *tf /= doc_length;
            }
        }
        
        // Update document frequencies
        for term in unique_terms {
            *self.document_frequencies.entry(term).or_insert(0) += 1;
        }
        
        self.term_frequencies.insert(doc_id, tf_map);
        self.total_documents += 1;
        
        // Clear IDF cache when new documents are added
        self.idf_cache.clear();
    }
    
    /// Calculate TF-IDF score for a term in a document with Phase 3 stop-motifs filtering
    pub fn tf_idf(&mut self, doc_id: &str, term: &str) -> f64 {
        let tf = self.term_frequencies
            .get(doc_id)
            .and_then(|tf_map| tf_map.get(term))
            .unwrap_or(&0.0)
            .clone();
        
        let idf = self.idf(term);
        let mut base_score = tf * idf;
        
        // Phase 3: Apply stop-motifs weight adjustment
        if let Some(ref cache) = self.stop_motif_cache {
            base_score = self.apply_stop_motif_adjustment(term, base_score, cache);
        }
        
        base_score
    }
    
    /// Apply Phase 3 stop-motifs weight adjustment
    fn apply_stop_motif_adjustment(&self, term: &str, base_score: f64, cache: &crate::io::cache::StopMotifCache) -> f64 {
        // Check if term matches any stop-motif pattern
        for stop_motif in &cache.token_grams {
            if self.term_matches_pattern(term, &stop_motif.pattern) {
                let adjusted_score = base_score * stop_motif.weight_multiplier;
                tracing::trace!("Phase 3 stop-motif adjustment: '{}' -> {:.3} (×{:.1})", 
                               term, adjusted_score, stop_motif.weight_multiplier);
                return adjusted_score;
            }
        }
        
        base_score
    }
    
    /// Check if a term matches a stop-motif pattern
    fn term_matches_pattern(&self, term: &str, pattern: &str) -> bool {
        // For k-gram patterns, check exact match or containment
        if pattern.contains(' ') {
            // Multi-token k-gram - check if term contains the pattern
            term.contains(pattern) || pattern.contains(term)
        } else {
            // Single token - exact match
            term == pattern
        }
    }
    
    /// Calculate IDF (Inverse Document Frequency) for a term
    pub fn idf(&mut self, term: &str) -> f64 {
        if let Some(&cached_idf) = self.idf_cache.get(term) {
            return cached_idf;
        }
        
        let df = self.document_frequencies.get(term).unwrap_or(&0);
        let idf = if *df > 0 && self.total_documents > 0 {
            (self.total_documents as f64 / *df as f64).ln() + 1.0
        } else {
            0.0
        };
        
        self.idf_cache.insert(term.to_string(), idf);
        idf
    }
    
    /// Get TF-IDF weighted vector for a document
    pub fn get_tfidf_vector(&mut self, doc_id: &str) -> HashMap<String, f64> {
        let mut vector = HashMap::new();
        
        if let Some(tf_map) = self.term_frequencies.get(doc_id) {
            let terms: Vec<String> = tf_map.keys().cloned().collect();
            for term in terms {
                let tfidf = self.tf_idf(doc_id, &term);
                if tfidf > 0.0 {
                    vector.insert(term, tfidf);
                }
            }
        }
        
        vector
    }
    
    /// Normalize a token according to language-agnostic rules
    fn normalize_token(&self, token: &str) -> String {
        let mut normalized = token.to_string();
        
        // Apply alpha-rename for local variables (simplified)
        if self.normalization_config.alpha_rename_locals {
            normalized = self.alpha_rename_local(&normalized);
        }
        
        // Bucket literals
        if self.normalization_config.bucket_literals {
            normalized = self.bucket_literal(&normalized);
        }
        
        normalized
    }
    
    /// Alpha-rename local variables (simplified implementation)
    fn alpha_rename_local(&self, token: &str) -> String {
        // Simple heuristic: if it looks like a local variable, normalize it
        if token.len() < 20 && token.chars().all(|c| c.is_alphanumeric() || c == '_') 
           && token.chars().any(|c| c.is_lowercase()) {
            return "LOCAL_VAR".to_string();
        }
        token.to_string()
    }
    
    /// Bucket literal values
    fn bucket_literal(&self, token: &str) -> String {
        // Numeric literals
        if token.parse::<f64>().is_ok() {
            if token.contains('.') {
                return "FLOAT_LIT".to_string();
            } else {
                return "INT_LIT".to_string();
            }
        }
        
        // String literals
        if (token.starts_with('"') && token.ends_with('"')) ||
           (token.starts_with('\'') && token.ends_with('\'')) {
            return "STR_LIT".to_string();
        }
        
        token.to_string()
    }
}

/// Configuration for language-agnostic normalization
#[derive(Debug, Clone)]
pub struct NormalizationConfig {
    /// Whether to alpha-rename local variables
    pub alpha_rename_locals: bool,
    
    /// Whether to bucket literal values
    pub bucket_literals: bool,
    
    /// Whether to strip comments
    pub strip_comments: bool,
    
    /// Whether to strip imports
    pub strip_imports: bool,
    
    /// Language-specific keywords to normalize
    pub language_keywords: HashSet<String>,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            alpha_rename_locals: true,
            bucket_literals: true,
            strip_comments: true,
            strip_imports: true,
            language_keywords: HashSet::new(),
        }
    }
}

/// PDG (Program Dependence Graph) Motif Analyzer
#[derive(Debug)]
pub struct PdgMotifAnalyzer {
    /// Cache of motif IDF scores
    motif_cache: HashMap<String, f64>,
    
    /// Structural patterns found in the codebase
    structural_patterns: Vec<StructuralPattern>,
    
    /// Basic block analyzer for structure validation
    basic_block_analyzer: BasicBlockAnalyzer,
    
    /// WL (Weisfeiler-Lehman) hash configuration
    wl_iterations: usize,
    
    /// Phase 3: Stop-motifs cache for PDG motif filtering
    stop_motif_cache: Option<Arc<crate::io::cache::StopMotifCache>>,
}

impl PdgMotifAnalyzer {
    /// Create a new PDG motif analyzer
    pub fn new(wl_iterations: usize) -> Self {
        Self {
            motif_cache: HashMap::new(),
            structural_patterns: Vec::new(),
            basic_block_analyzer: BasicBlockAnalyzer::new(),
            wl_iterations,
            stop_motif_cache: None,
        }
    }
    
    /// Set the stop-motifs cache for Phase 3 PDG motif filtering
    pub fn set_stop_motif_cache(&mut self, cache: Arc<crate::io::cache::StopMotifCache>) {
        let cache_info = format!("{} cached motif patterns", cache.pdg_motifs.len());
        self.stop_motif_cache = Some(cache);
        tracing::info!("Phase 3 PDG motif filtering enabled: {}", cache_info);
    }
    
    /// Extract PDG motifs from code with comprehensive analysis
    pub fn extract_motifs(&mut self, code: &str, entity_id: &str) -> Vec<PdgMotif> {
        let mut motifs = Vec::new();
        
        // Analyze basic blocks
        let basic_blocks = self.basic_block_analyzer.analyze(code);
        
        // Extract control flow motifs (branches, loops)
        for block in &basic_blocks {
            if let Some(motif) = self.extract_control_flow_motif(block) {
                motifs.push(motif);
            }
            
            // Extract call graph motifs
            if let Some(motif) = self.extract_call_graph_motif(block) {
                motifs.push(motif);
            }
            
            // Extract assignment patterns
            if let Some(motif) = self.extract_assignment_motif(block) {
                motifs.push(motif);
            }
        }
        
        // Extract data dependency motifs between blocks
        let data_motifs = self.extract_data_dependency_motifs(code, &basic_blocks);
        motifs.extend(data_motifs);
        
        // Extract higher-level structural patterns
        let structural_motifs = self.extract_structural_patterns(&basic_blocks);
        motifs.extend(structural_motifs);
        
        // Generate WL hashes for each motif
        for motif in &mut motifs {
            motif.wl_hash = self.compute_wl_hash(&motif.structure, entity_id);
            motif.motif_category = self.categorize_motif(&motif.motif_type, &motif.structure);
        }
        
        motifs
    }
    
    /// Extract call graph motifs from a basic block
    fn extract_call_graph_motif(&self, block: &BasicBlock) -> Option<PdgMotif> {
        if !block.has_external_calls {
            return None;
        }
        
        let calls = self.count_external_calls(block);
        if calls == 0 {
            return None;
        }
        
        Some(PdgMotif {
            motif_type: MotifType::CallGraph,
            structure: format!("calls:{}", calls),
            complexity: calls,
            wl_hash: String::new(),
            frequency: 1,
            motif_category: MotifCategory::Call,
        })
    }
    
    /// Extract assignment pattern motifs
    fn extract_assignment_motif(&self, block: &BasicBlock) -> Option<PdgMotif> {
        let assignments = self.count_assignments(block);
        if assignments == 0 {
            return None;
        }
        
        Some(PdgMotif {
            motif_type: MotifType::DataFlow,
            structure: format!("assign:{}", assignments),
            complexity: assignments,
            wl_hash: String::new(),
            frequency: 1,
            motif_category: MotifCategory::Assign,
        })
    }
    
    /// Extract structural patterns from basic blocks
    fn extract_structural_patterns(&self, blocks: &[BasicBlock]) -> Vec<PdgMotif> {
        let mut motifs = Vec::new();
        
        // Look for sequential control patterns
        for window in blocks.windows(2) {
            let pattern = format!("{:?}->{:?}", window[0].control_type, window[1].control_type);
            motifs.push(PdgMotif {
                motif_type: MotifType::ControlFlow,
                structure: pattern,
                complexity: 2,
                wl_hash: String::new(),
                frequency: 1,
                motif_category: self.categorize_control_pattern(&window[0].control_type, &window[1].control_type),
            });
        }
        
        // Look for nested patterns (simplified)
        let nesting_depth = self.calculate_nesting_depth(blocks);
        if nesting_depth > 1 {
            motifs.push(PdgMotif {
                motif_type: MotifType::ControlFlow,
                structure: format!("nested:{}", nesting_depth),
                complexity: nesting_depth,
                wl_hash: String::new(),
                frequency: 1,
                motif_category: MotifCategory::Branch,
            });
        }
        
        motifs
    }
    
    /// Count external calls in a basic block
    fn count_external_calls(&self, block: &BasicBlock) -> usize {
        block.lines.iter()
            .map(|line| self.count_calls_in_line(line))
            .sum()
    }
    
    /// Count assignments in a basic block
    fn count_assignments(&self, block: &BasicBlock) -> usize {
        block.lines.iter()
            .filter(|line| self.is_assignment_line(line))
            .count()
    }
    
    /// Count function calls in a line
    fn count_calls_in_line(&self, line: &str) -> usize {
        line.matches('(').count()
    }
    
    /// Check if line contains assignment
    fn is_assignment_line(&self, line: &str) -> bool {
        line.contains('=') && !line.contains("==") && !line.contains("!=") && !line.contains("<=") && !line.contains(">=")
    }
    
    /// Calculate nesting depth of control structures
    fn calculate_nesting_depth(&self, blocks: &[BasicBlock]) -> usize {
        let mut max_depth = 0;
        let mut current_depth = 0;
        
        for block in blocks {
            match block.control_type {
                ControlType::Conditional | ControlType::Loop => {
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth);
                },
                _ => {
                    if current_depth > 0 {
                        current_depth -= 1;
                    }
                }
            }
        }
        
        max_depth
    }
    
    /// Categorize control flow patterns
    fn categorize_control_pattern(&self, first: &ControlType, second: &ControlType) -> MotifCategory {
        match (first, second) {
            (ControlType::Conditional, _) => MotifCategory::Branch,
            (ControlType::Loop, _) => MotifCategory::Loop,
            (_, ControlType::Loop) => MotifCategory::Loop,
            _ => MotifCategory::Assign,
        }
    }
    
    /// Categorize a motif based on its type and structure
    fn categorize_motif(&self, motif_type: &MotifType, structure: &str) -> MotifCategory {
        match motif_type {
            MotifType::ControlFlow => {
                if structure.contains("Loop") || structure.contains("for") || structure.contains("while") {
                    MotifCategory::Loop
                } else if structure.contains("Conditional") || structure.contains("if") {
                    MotifCategory::Branch
                } else {
                    MotifCategory::Branch
                }
            },
            MotifType::CallGraph => MotifCategory::Call,
            MotifType::DataFlow => {
                if structure.contains("assign") {
                    MotifCategory::Assign
                } else {
                    MotifCategory::Phi
                }
            }
        }
    }
    
    /// Extract control flow motif from a basic block
    fn extract_control_flow_motif(&self, block: &BasicBlock) -> Option<PdgMotif> {
        if block.control_type == ControlType::None {
            return None;
        }
        
        let category = match block.control_type {
            ControlType::Conditional => MotifCategory::Branch,
            ControlType::Loop => MotifCategory::Loop,
            ControlType::Exception => MotifCategory::Branch,
            ControlType::None => return None,
        };
        
        Some(PdgMotif {
            motif_type: MotifType::ControlFlow,
            structure: block.structure.clone(),
            complexity: block.cyclomatic_complexity,
            wl_hash: String::new(), // Will be filled later
            frequency: 1,
            motif_category: category,
        })
    }
    
    /// Extract data dependency motifs
    fn extract_data_dependency_motifs(&self, _code: &str, blocks: &[BasicBlock]) -> Vec<PdgMotif> {
        let mut motifs = Vec::new();
        
        // Look for data flow patterns between blocks
        for (i, block) in blocks.iter().enumerate() {
            for j in (i + 1)..blocks.len() {
                let other_block = &blocks[j];
                
                if self.has_data_dependency(block, other_block) {
                    motifs.push(PdgMotif {
                        motif_type: MotifType::DataFlow,
                        structure: format!("{}→{}", block.block_id, other_block.block_id),
                        complexity: 2,
                        wl_hash: String::new(),
                        frequency: 1,
                        motif_category: MotifCategory::Phi,
                    });
                }
            }
        }
        
        motifs
    }
    
    /// Check if there's a data dependency between blocks
    fn has_data_dependency(&self, _block1: &BasicBlock, _block2: &BasicBlock) -> bool {
        // Simplified heuristic for data dependency detection
        // In a real implementation, this would analyze variable definitions and uses
        true // Placeholder
    }
    
    /// Compute Weisfeiler-Lehman hash for a structure
    fn compute_wl_hash(&self, structure: &str, entity_id: &str) -> String {
        let mut hasher = AHasher::default();
        entity_id.hash(&mut hasher);
        structure.hash(&mut hasher);
        self.wl_iterations.hash(&mut hasher);
        
        format!("wl_{:x}", hasher.finish())
    }
    
    /// Calculate rarity gain for motifs with Phase 3 stop-motifs filtering
    pub fn calculate_rarity_gain(&mut self, motifs: &[PdgMotif]) -> f64 {
        let mut total_weighted_idf = 0.0;
        let mut count = 0;
        let mut stop_motif_adjustments = 0;
        
        for motif in motifs {
            let base_idf = self.get_motif_idf(&motif.wl_hash);
            
            // Phase 3: Apply stop-motifs adjustment
            let adjusted_idf = if let Some(ref cache) = self.stop_motif_cache {
                self.apply_motif_stop_adjustment(motif, base_idf, cache, &mut stop_motif_adjustments)
            } else {
                base_idf
            };
            
            total_weighted_idf += adjusted_idf;
            count += 1;
        }
        
        if stop_motif_adjustments > 0 {
            tracing::debug!("Phase 3: Applied stop-motif adjustments to {}/{} motifs", 
                           stop_motif_adjustments, count);
        }
        
        if count > 0 {
            total_weighted_idf / count as f64
        } else {
            1.0
        }
    }
    
    /// Apply Phase 3 stop-motif adjustment to PDG motif IDF score
    fn apply_motif_stop_adjustment(&self, motif: &PdgMotif, base_idf: f64, 
                                  cache: &crate::io::cache::StopMotifCache, 
                                  adjustment_count: &mut usize) -> f64 {
        // Check if motif matches cached stop-motif patterns
        for stop_motif in &cache.pdg_motifs {
            if self.motif_matches_stop_pattern(motif, stop_motif) {
                let adjusted_idf = base_idf * stop_motif.weight_multiplier;
                tracing::trace!("Phase 3 motif stop-adjustment: {:?} '{}' -> {:.3} (×{:.1})", 
                               motif.motif_category, motif.structure, adjusted_idf, stop_motif.weight_multiplier);
                *adjustment_count += 1;
                return adjusted_idf;
            }
        }
        
        base_idf
    }
    
    /// Check if a PDG motif matches a stop-motif pattern
    fn motif_matches_stop_pattern(&self, motif: &PdgMotif, stop_motif: &crate::io::cache::StopMotifEntry) -> bool {
        use crate::io::cache::PatternCategory;
        
        // Match by category and pattern
        let motif_category_matches = match (&motif.motif_category, &stop_motif.category) {
            (MotifCategory::Branch, PatternCategory::ControlFlow) => stop_motif.pattern.contains("branch"),
            (MotifCategory::Loop, PatternCategory::ControlFlow) => stop_motif.pattern.contains("loop"),
            (MotifCategory::Call, PatternCategory::FunctionCall) => stop_motif.pattern.contains("call"),
            (MotifCategory::Assign, PatternCategory::Assignment) => stop_motif.pattern.contains("assign"),
            (MotifCategory::Phi, PatternCategory::DataStructure) => true,
            (MotifCategory::Ret, PatternCategory::Boilerplate) => stop_motif.pattern.contains("return"),
            _ => false,
        };
        
        // Also check structure similarity
        let structure_matches = motif.structure.contains(&stop_motif.pattern) || 
                               stop_motif.pattern.contains(&motif.structure);
        
        motif_category_matches || structure_matches
    }
    
    /// Get IDF score for a motif
    fn get_motif_idf(&mut self, motif_hash: &str) -> f64 {
        if let Some(&idf) = self.motif_cache.get(motif_hash) {
            return idf;
        }
        
        // Calculate IDF based on motif frequency across codebase
        // This is a simplified calculation - in practice, would use corpus statistics
        let idf = 2.0 + (motif_hash.len() as f64 * 0.1).ln();
        self.motif_cache.insert(motif_hash.to_string(), idf);
        idf
    }
}

/// Structural pattern in code
#[derive(Debug, Clone)]
pub struct StructuralPattern {
    pub pattern_id: String,
    pub pattern_type: String,
    pub frequency: usize,
    pub examples: Vec<String>,
}

/// PDG Motif representing a structural pattern
#[derive(Debug, Clone)]
pub struct PdgMotif {
    pub motif_type: MotifType,
    pub structure: String,
    pub complexity: usize,
    pub wl_hash: String,
    pub frequency: usize,
    pub motif_category: MotifCategory,
}

/// Type of PDG motif
#[derive(Debug, Clone, PartialEq)]
pub enum MotifType {
    ControlFlow,
    DataFlow,
    CallGraph,
}

/// Category of motif for fine-grained analysis
#[derive(Debug, Clone, PartialEq)]
pub enum MotifCategory {
    Branch,   // Conditional patterns
    Loop,     // Iteration patterns  
    Call,     // Function call patterns
    Assign,   // Assignment patterns
    Phi,      // Data merge patterns
    Ret,      // Return patterns
}

/// Basic block analyzer for structure validation with match region analysis
#[derive(Debug)]
pub struct BasicBlockAnalyzer {
    block_counter: usize,
}

impl BasicBlockAnalyzer {
    pub fn new() -> Self {
        Self { block_counter: 0 }
    }
    
    /// Analyze code to extract basic blocks
    pub fn analyze(&mut self, code: &str) -> Vec<BasicBlock> {
        let mut blocks = Vec::new();
        self.block_counter = 0;
        
        let lines: Vec<&str> = code.lines().collect();
        let mut current_block = BasicBlock::new(self.next_block_id());
        
        for (line_idx, line) in lines.iter().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            
            current_block.lines.push(line.to_string());
            current_block.line_ranges.push(line_idx);
            
            // Detect control flow changes
            if self.is_control_flow_line(line) {
                current_block.control_type = self.detect_control_type(line);
                current_block.cyclomatic_complexity += 1;
                current_block.has_external_calls |= self.has_external_call(line);
            }
            
            // End block on certain patterns
            if self.ends_block(line) {
                current_block.structure = format!("{:?}:{}", 
                    current_block.control_type, current_block.lines.len());
                blocks.push(current_block);
                current_block = BasicBlock::new(self.next_block_id());
            }
        }
        
        // Add final block
        if !current_block.lines.is_empty() {
            current_block.structure = format!("{:?}:{}", 
                current_block.control_type, current_block.lines.len());
            blocks.push(current_block);
        }
        
        blocks
    }
    
    /// Compute matched blocks between two sets of basic blocks
    pub fn compute_matched_blocks(&self, blocks1: &[BasicBlock], blocks2: &[BasicBlock], 
                                  match_start1: usize, match_end1: usize,
                                  match_start2: usize, match_end2: usize) -> StructuralMatchInfo {
        let mut matched_blocks_1 = 0;
        let mut matched_blocks_2 = 0;
        let mut shared_external_calls = HashSet::new();
        let mut external_calls_1 = HashSet::new();
        let mut external_calls_2 = HashSet::new();
        
        // Check which blocks from entity 1 overlap with match region
        for block in blocks1 {
            if self.block_overlaps_region(block, match_start1, match_end1) {
                matched_blocks_1 += 1;
                if block.has_external_calls {
                    let calls = self.extract_external_calls(block);
                    external_calls_1.extend(calls);
                }
            }
        }
        
        // Check which blocks from entity 2 overlap with match region  
        for block in blocks2 {
            if self.block_overlaps_region(block, match_start2, match_end2) {
                matched_blocks_2 += 1;
                if block.has_external_calls {
                    let calls = self.extract_external_calls(block);
                    external_calls_2.extend(calls);
                }
            }
        }
        
        // Calculate shared external calls
        for call in &external_calls_1 {
            if external_calls_2.contains(call) {
                shared_external_calls.insert(call.clone());
            }
        }
        
        // Calculate Jaccard similarity for external calls
        let union_size = external_calls_1.union(&external_calls_2).count();
        let external_call_jaccard = if union_size > 0 {
            shared_external_calls.len() as f64 / union_size as f64
        } else {
            1.0 // No external calls on either side
        };
        
        StructuralMatchInfo {
            matched_blocks_1,
            matched_blocks_2,
            total_blocks_1: blocks1.len(),
            total_blocks_2: blocks2.len(),
            external_call_jaccard,
            shared_external_calls: shared_external_calls.len(),
            total_external_calls_1: external_calls_1.len(),
            total_external_calls_2: external_calls_2.len(),
        }
    }
    
    /// Check if a basic block overlaps with a match region
    fn block_overlaps_region(&self, block: &BasicBlock, match_start: usize, match_end: usize) -> bool {
        if block.line_ranges.is_empty() {
            return false;
        }
        
        let block_start = *block.line_ranges.first().unwrap();
        let block_end = *block.line_ranges.last().unwrap();
        
        // Check for overlap: block overlaps if any part is within match region
        !(block_end < match_start || block_start > match_end)
    }
    
    /// Extract external function calls from a basic block
    fn extract_external_calls(&self, block: &BasicBlock) -> Vec<String> {
        let mut calls = Vec::new();
        
        for line in &block.lines {
            // Simple pattern matching for function calls
            if let Some(call) = self.extract_function_call(line) {
                calls.push(call);
            }
        }
        
        calls
    }
    
    /// Extract function call name from a line of code using text parsing (tree-sitter replacement)
    fn extract_function_call(&self, line: &str) -> Option<String> {
        // Match patterns like: function_name(, obj.method(, Module::function(
        // This replaces regex with simple text parsing
        
        let line = line.trim();
        
        // Find opening parenthesis
        if let Some(paren_pos) = line.find('(') {
            // Look backward from the parenthesis to find the function name
            let before_paren = &line[..paren_pos].trim_end();
            
            // Split by common separators and take the last part
            let parts: Vec<&str> = before_paren
                .split(|c: char| c.is_whitespace() || c == '=' || c == ',' || c == ';')
                .collect();
            
            if let Some(last_part) = parts.last() {
                let last_part = last_part.trim();
                
                // Extract the function identifier (handle obj.method, Module::function, etc.)
                if !last_part.is_empty() && is_valid_function_identifier(last_part) {
                    return Some(last_part.to_string());
                }
            }
        }
        
        None
    }
    
    /// Check if line contains external calls
    fn has_external_call(&self, line: &str) -> bool {
        // Look for function call patterns
        line.contains('(') && (
            line.contains("::") || // Rust module calls
            line.contains(".") ||  // Method calls  
            (line.contains("(") && !line.trim_start().starts_with("//")) // General function calls
        )
    }
    
    fn next_block_id(&mut self) -> String {
        let id = format!("block_{}", self.block_counter);
        self.block_counter += 1;
        id
    }
}

/// Helper function to validate function identifiers (replacement for regex pattern matching)
fn is_valid_function_identifier(identifier: &str) -> bool {
    if identifier.is_empty() {
        return false;
    }
    
    // Check for valid function call patterns: function_name, obj.method, Module::function
    let parts: Vec<&str> = if identifier.contains("::") {
        identifier.split("::").collect()
    } else if identifier.contains('.') {
        identifier.split('.').collect()
    } else {
        vec![identifier]
    };
    
    // Each part should be a valid identifier
    for part in parts {
        if part.is_empty() {
            return false;
        }
        
        // First character should be letter or underscore
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            if !first.is_ascii_alphabetic() && first != '_' {
                return false;
            }
        } else {
            return false;
        }
        
        // Remaining characters should be alphanumeric or underscore
        for c in chars {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return false;
            }
        }
    }
    
    true
}

impl BasicBlockAnalyzer {
    fn is_control_flow_line(&self, line: &str) -> bool {
        line.contains("if ") || line.contains("for ") || line.contains("while ") ||
        line.contains("match ") || line.contains("try ") || line.contains("catch ")
    }
    
    fn detect_control_type(&self, line: &str) -> ControlType {
        if line.contains("if ") {
            ControlType::Conditional
        } else if line.contains("for ") || line.contains("while ") {
            ControlType::Loop
        } else if line.contains("try ") || line.contains("catch ") {
            ControlType::Exception
        } else {
            ControlType::None
        }
    }
    
    fn ends_block(&self, line: &str) -> bool {
        line.ends_with('{') || line.ends_with('}') || line.ends_with(';')
    }
}

/// Basic block representation
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub block_id: String,
    pub lines: Vec<String>,
    pub line_ranges: Vec<usize>,
    pub control_type: ControlType,
    pub cyclomatic_complexity: usize,
    pub structure: String,
    pub has_external_calls: bool,
}

impl BasicBlock {
    fn new(block_id: String) -> Self {
        Self {
            block_id,
            lines: Vec::new(),
            line_ranges: Vec::new(),
            control_type: ControlType::None,
            cyclomatic_complexity: 1,
            structure: String::new(),
            has_external_calls: false,
        }
    }
}

/// Structural analysis information for a match pair
#[derive(Debug, Clone)]
pub struct StructuralMatchInfo {
    pub matched_blocks_1: usize,
    pub matched_blocks_2: usize,
    pub total_blocks_1: usize,
    pub total_blocks_2: usize,
    pub external_call_jaccard: f64,
    pub shared_external_calls: usize,
    pub total_external_calls_1: usize,
    pub total_external_calls_2: usize,
}

/// Control flow type
#[derive(Debug, Clone, PartialEq)]
pub enum ControlType {
    None,
    Conditional,
    Loop,
    Exception,
}

/// Weighted MinHash implementation for TF-IDF weighted similarity
#[derive(Debug)]
pub struct WeightedMinHash {
    /// Hash functions for MinHash
    hash_functions: Vec<HashFunction>,
    
    /// TF-IDF weights for terms
    weights: HashMap<String, f64>,
    
    /// Size of the signature
    signature_size: usize,
    
    /// Random seeds for hash functions
    seeds: Vec<u64>,
}

impl WeightedMinHash {
    /// Create a new weighted MinHash
    pub fn new(signature_size: usize, weights: HashMap<String, f64>) -> Self {
        let seeds: Vec<u64> = (0..signature_size).map(|i| i as u64 * 0x9e3779b97f4a7c15).collect();
        let hash_functions = seeds.iter().map(|&seed| HashFunction::new(seed)).collect();
        
        Self {
            hash_functions,
            weights,
            signature_size,
            seeds,
        }
    }
    
    /// Generate weighted MinHash signature
    pub fn generate_signature(&self, tokens: &[String]) -> WeightedSignature {
        let mut signature = vec![f64::INFINITY; self.signature_size];
        
        for token in tokens {
            let weight = self.weights.get(token).unwrap_or(&1.0);
            
            // Skip tokens with very low weight (stop motifs)
            if *weight < 0.1 {
                continue;
            }
            
            for (i, hash_func) in self.hash_functions.iter().enumerate() {
                let hash_value = hash_func.hash(token);
                let weighted_hash = hash_value as f64 / weight;
                
                if weighted_hash < signature[i] {
                    signature[i] = weighted_hash;
                }
            }
        }
        
        WeightedSignature {
            signature,
            size: self.signature_size,
        }
    }
    
    /// Update weights with new TF-IDF scores
    pub fn update_weights(&mut self, new_weights: HashMap<String, f64>) {
        self.weights.extend(new_weights);
    }
}

/// Hash function for MinHash
#[derive(Debug)]
pub struct HashFunction {
    seed: u64,
}

impl HashFunction {
    fn new(seed: u64) -> Self {
        Self { seed }
    }
    
    fn hash(&self, data: &str) -> u64 {
        let mut hasher = AHasher::default();
        self.seed.hash(&mut hasher);
        data.hash(&mut hasher);
        hasher.finish()
    }
}

/// Weighted MinHash signature
#[derive(Debug, Clone)]
pub struct WeightedSignature {
    pub signature: Vec<f64>,
    pub size: usize,
}

impl WeightedSignature {
    /// Calculate weighted Jaccard similarity
    pub fn jaccard_similarity(&self, other: &WeightedSignature) -> f64 {
        if self.size != other.size {
            return 0.0;
        }
        
        let mut matches = 0;
        let epsilon = 1e-10;
        
        for (a, b) in self.signature.iter().zip(other.signature.iter()) {
            if (a - b).abs() < epsilon {
                matches += 1;
            }
        }
        
        matches as f64 / self.size as f64
    }
}

/// Comprehensive clone detection feature extractor with Phase 2 structural gates
#[derive(Debug)]
pub struct ComprehensiveCloneDetector {
    /// Feature definitions
    features: Vec<FeatureDefinition>,
    
    /// TF-IDF analyzer
    tfidf_analyzer: Arc<std::sync::Mutex<TfIdfAnalyzer>>,
    
    /// PDG motif analyzer
    pdg_analyzer: Arc<std::sync::Mutex<PdgMotifAnalyzer>>,
    
    /// Weighted MinHash system
    weighted_minhash: Arc<WeightedMinHash>,
    
    /// Phase 2 Structural Gate Analyzer 
    structural_gates: Arc<std::sync::Mutex<StructuralGateAnalyzer>>,
    
    /// Configuration
    config: DedupeConfig,
    
    /// Auto-calibration engine
    auto_calibration: AutoCalibrationEngine,
    
    /// Phase 4: Payoff ranking system
    payoff_ranking: PayoffRankingSystem,
    
    /// Phase 2 filtering statistics
    phase2_stats: Phase2FilteringStats,
    
    /// Phase 3: Stop-motifs cache manager
    stop_motif_cache_manager: Option<Arc<StopMotifCacheManager>>,
}

impl ComprehensiveCloneDetector {
    /// Create a new comprehensive clone detector with Phase 2 structural gates
    pub fn new(config: DedupeConfig) -> Self {
        let structural_config = StructuralGateConfig {
            require_blocks: config.require_distinct_blocks,
            min_shared_motifs: 2,
            external_call_jaccard_threshold: config.adaptive.external_call_jaccard_threshold,
            io_penalty_multiplier: 0.7,
            wl_iterations: config.adaptive.wl_iterations,
        };
        
        let mut detector = Self {
            features: Vec::new(),
            tfidf_analyzer: Arc::new(std::sync::Mutex::new(
                TfIdfAnalyzer::new(NormalizationConfig::default())
            )),
            pdg_analyzer: Arc::new(std::sync::Mutex::new(
                PdgMotifAnalyzer::new(config.adaptive.wl_iterations)
            )),
            weighted_minhash: Arc::new(
                WeightedMinHash::new(128, HashMap::new())
            ),
            structural_gates: Arc::new(std::sync::Mutex::new(
                StructuralGateAnalyzer::new(structural_config)
            )),
            config,
            auto_calibration: AutoCalibrationEngine::new(),
            payoff_ranking: PayoffRankingSystem::new(),
            phase2_stats: Phase2FilteringStats::new(),
            stop_motif_cache_manager: None,
        };
        
        detector.initialize_features();
        detector
    }
    
    /// Apply Phase 2 structural gates to clone candidates
    pub fn filter_candidates_phase2(&mut self, candidates: Vec<CloneCandidate>, 
                                    code_mapping: &HashMap<String, String>) -> Vec<FilteredCloneCandidate> {
        let mut filtered_candidates = Vec::new();
        let mut rejected_stats = RejectionStats::new();
        
        tracing::info!("Phase 2: Applying structural gates to {} candidates", candidates.len());
        let candidates_len = candidates.len();
        
        for candidate in candidates {
            // Get source code for both entities
            let code1 = match code_mapping.get(&candidate.entity_id) {
                Some(code) => code,
                None => {
                    tracing::warn!("No code found for entity: {}", candidate.entity_id);
                    continue;
                }
            };
            
            let code2 = match code_mapping.get(&candidate.similar_entity_id) {
                Some(code) => code,
                None => {
                    tracing::warn!("No code found for entity: {}", candidate.similar_entity_id);
                    continue;
                }
            };
            
            // Apply structural gates
            let mut gates = self.structural_gates.lock().unwrap();
            match gates.apply_structural_gates(&candidate, code1, code2) {
                Some(filtered) => {
                    filtered_candidates.push(filtered);
                },
                None => {
                    rejected_stats.increment_structural_rejection();
                }
            }
        }
        
        // Update statistics
        self.phase2_stats.update_filtering_round(candidates_len, filtered_candidates.len(), rejected_stats);
        
        tracing::info!("Phase 2 complete: {} candidates passed structural gates ({} rejected)", 
                  filtered_candidates.len(), candidates_len - filtered_candidates.len());
        
        self.phase2_stats.log_comprehensive_stats();
        
        filtered_candidates
    }
    
    /// Get Phase 2 filtering statistics
    pub fn get_phase2_stats(&self) -> &Phase2FilteringStats {
        &self.phase2_stats
    }
    
    /// Detect clones with full 4-phase denoising pipeline
    pub async fn detect_clones_with_denoising(&mut self, entities: &[&CodeEntity]) -> Result<Vec<CloneCandidate>> {
        use crate::api::results::{CloneAnalysisResults, PhaseFilteringStats, CloneAnalysisPerformance};
        
        let start_time = std::time::Instant::now();
        tracing::info!("Starting comprehensive clone detection with denoising for {} entities", entities.len());
        
        // Initialize empty results
        let mut candidates = Vec::new();
        let mut phase_stats = PhaseFilteringStats {
            phase1_weighted_signature: 0,
            phase2_structural_gates: 0,
            phase3_stop_motifs_filter: 0,
            phase4_payoff_ranking: 0,
        };
        
        // Phase 1: Generate initial candidates using weighted signatures
        for (i, entity1) in entities.iter().enumerate() {
            for entity2 in entities.iter().skip(i + 1) {
                // Generate weighted signature for both entities
                let tokens1 = self.extract_normalized_tokens(&entity1.source_code)?;
                let tokens2 = self.extract_normalized_tokens(&entity2.source_code)?;
                
                let sig1 = self.weighted_minhash.generate_signature(&tokens1);
                let sig2 = self.weighted_minhash.generate_signature(&tokens2);
                
                // Calculate similarity
                let similarity = sig1.jaccard_similarity(&sig2);
                
                if similarity > 0.3 { // Threshold for initial candidate selection
                    let candidate = CloneCandidate {
                        entity_id: entity1.id.clone(),
                        similar_entity_id: entity2.id.clone(),
                        score: similarity,
                        saved_tokens: tokens1.len().min(tokens2.len()),
                        rarity_gain: 0.0, // Will be calculated later
                        matched_blocks: 0, // Will be calculated later
                        total_blocks: 1,
                        structural_motifs: 0, // Will be calculated later  
                        total_motifs: 1,
                        live_reach_boost: 0.0, // Will be calculated later
                    };
                    candidates.push(candidate);
                }
            }
        }
        phase_stats.phase1_weighted_signature = candidates.len();
        tracing::info!("Phase 1: Found {} candidates from weighted signatures", candidates.len());
        
        // Phase 2: Apply structural gates
        let code_mapping: HashMap<String, String> = entities.iter()
            .map(|e| (e.id.clone(), e.source_code.clone()))
            .collect();
        
        let filtered_candidates = self.filter_candidates_phase2(candidates, &code_mapping);
        phase_stats.phase2_structural_gates = filtered_candidates.len();
        tracing::info!("Phase 2: {} candidates passed structural gates", filtered_candidates.len());
        
        // Convert FilteredCloneCandidate back to CloneCandidate for Phase 4
        let candidates_for_phase4: Vec<CloneCandidate> = filtered_candidates.into_iter()
            .map(|fc| CloneCandidate {
                entity_id: fc.original.entity_id,
                similar_entity_id: fc.original.similar_entity_id,
                score: fc.adjusted_score, // Use adjusted score from filtering
                saved_tokens: fc.original.saved_tokens,
                rarity_gain: fc.original.rarity_gain,
                matched_blocks: fc.original.matched_blocks,
                total_blocks: fc.original.total_blocks,
                structural_motifs: fc.original.structural_motifs,
                total_motifs: fc.original.total_motifs,
                live_reach_boost: fc.original.live_reach_boost,
            })
            .collect();
        
        // Phase 3: Apply stop-motifs filtering (simulated for now)
        phase_stats.phase3_stop_motifs_filter = candidates_for_phase4.len();
        
        // Phase 4: Auto-calibration and payoff ranking
        let _ranked_candidates = self.apply_phase4_processing(candidates_for_phase4.clone(), true)?;
        phase_stats.phase4_payoff_ranking = candidates_for_phase4.len(); // Simplified
        
        let elapsed = start_time.elapsed();
        tracing::info!("Clone detection completed in {:?}", elapsed);
        
        Ok(candidates_for_phase4)
    }
    
    /// Phase 4: Apply auto-calibration and payoff ranking to clone candidates
    pub fn apply_phase4_processing(&mut self, candidates: Vec<CloneCandidate>, 
                                   use_auto_denoise: bool) -> Result<Vec<RankedCloneCandidate>> {
        tracing::info!("Starting Phase 4: Auto-calibration and payoff ranking for {} candidates", 
                      candidates.len());
        
        let processed_candidates = if use_auto_denoise {
            // Apply auto-calibration with 80% quality target
            let calibration_result = match self.auto_calibration.cache_manager.load_cached_calibration()? {
                Some(cached) => {
                    tracing::info!("Using cached calibration results");
                    cached
                },
                None => {
                    tracing::info!("Running auto-calibration sweep");
                    self.auto_calibration.auto_calibrate_enhanced(&candidates, 0.8)?
                }
            };
            
            // Apply calibrated thresholds
            tracing::info!("Applying calibrated thresholds: fragmentarity={:.2}, structure_ratio={:.2}, uniqueness={:.2}", 
                          calibration_result.thresholds.fragmentarity_threshold,
                          calibration_result.thresholds.structure_ratio_threshold,
                          calibration_result.thresholds.uniqueness_threshold);
            
            calibration_result.thresholds.apply_filtering(candidates)
        } else {
            // Use original candidates without auto-calibration
            candidates
        };
        
        // Apply hard filtering floors
        let candidates_after_floors = processed_candidates.into_iter()
            .filter(|candidate| {
                candidate.saved_tokens >= 100 && candidate.rarity_gain >= 1.2
            })
            .collect();
        
        // Apply payoff ranking
        let ranked_candidates = self.payoff_ranking.rank_candidates(candidates_after_floors);
        
        tracing::info!("Phase 4 complete: {} candidates ranked by payoff score", 
                      ranked_candidates.len());
        
        // Log top 5 candidates for debugging
        for (i, ranked) in ranked_candidates.iter().take(5).enumerate() {
            tracing::debug!("Top {} candidate: payoff_score={:.2}, saved_tokens={}, rarity_gain={:.2}", 
                           i + 1, ranked.payoff_score, ranked.candidate.saved_tokens, 
                           ranked.candidate.rarity_gain);
        }
        
        Ok(ranked_candidates)
    }
    
    /// Configure payoff ranking system with live reach data
    pub fn set_live_reach_data(&mut self, data: HashMap<String, f64>) {
        let data_len = data.len();
        let current_idf = self.payoff_ranking.idf_statistics.clone();
        self.payoff_ranking = PayoffRankingSystem::new()
            .with_live_reach_data(data)
            .with_idf_statistics(current_idf);
        
        tracing::info!("Live reach data configured for {} entities", data_len);
    }
    
    /// Configure payoff ranking system with IDF statistics
    pub fn set_idf_statistics(&mut self, stats: IdfStatistics) {
        let current_live_reach = self.payoff_ranking.live_reach_data.clone();
        let current_hard_floors = self.payoff_ranking.hard_floors.clone();
        self.payoff_ranking = PayoffRankingSystem {
            live_reach_data: current_live_reach,
            idf_statistics: stats,
            hard_floors: current_hard_floors,
        };
        tracing::info!("IDF statistics configured: repo_median_idf={:.2}, mean_idf_overall={:.2}", 
                      self.payoff_ranking.idf_statistics.repo_median_idf,
                      self.payoff_ranking.idf_statistics.mean_idf_overall);
    }
    
    /// Get calibrated thresholds for external use
    pub fn get_calibrated_thresholds(&self) -> Result<Option<AdaptiveThresholds>> {
        match self.auto_calibration.cache_manager.load_cached_calibration()? {
            Some(result) => Ok(Some(result.thresholds)),
            None => Ok(None),
        }
    }
    
    /// Force recalibration (ignores cache)
    pub fn force_recalibration(&mut self, candidates: &[CloneCandidate]) -> Result<CalibrationResult> {
        self.auto_calibration.auto_calibrate_enhanced(candidates, 0.8)
    }
    
    /// Enable Phase 3 stop-motifs cache with automatic boilerplate detection
    pub fn enable_stop_motif_cache<P: AsRef<std::path::Path>>(&mut self, cache_dir: P) -> Result<()> {
        let policy = CacheRefreshPolicy {
            max_age_days: 7,
            change_threshold_percent: 5.0,
            stop_motif_percentile: self.config.adaptive.stop_motif_percentile * 100.0,
            weight_multiplier: 0.2, // Down-weight stop-motifs to 20% of original weight
            k_gram_size: 9,
        };
        
        let cache_manager = Arc::new(StopMotifCacheManager::new(cache_dir, policy));
        self.stop_motif_cache_manager = Some(cache_manager);
        
        tracing::info!("Phase 3 stop-motifs cache enabled with {:.1}% threshold", 
                      self.config.adaptive.stop_motif_percentile * 100.0);
        Ok(())
    }
    
    /// Initialize stop-motifs cache from codebase (call before analysis)
    pub async fn initialize_stop_motifs_cache(&mut self, context: &ExtractionContext) -> Result<()> {
        if let Some(ref cache_manager) = self.stop_motif_cache_manager {
            // Build codebase info from extraction context
            let codebase_info = self.build_codebase_info(context)?;
            
            // Get or create the cache
            let cache = cache_manager.get_cache(&codebase_info)?;
            
            // Apply cache to analyzers
            {
                let mut tfidf = self.tfidf_analyzer.lock().unwrap();
                tfidf.set_stop_motif_cache(cache.clone());
            }
            
            {
                let mut pdg = self.pdg_analyzer.lock().unwrap();
                pdg.set_stop_motif_cache(cache.clone());
            }
            
            tracing::info!("Phase 3 stop-motifs cache initialized: {} functions analyzed, {} patterns cached", 
                          cache.mining_stats.functions_analyzed, cache.mining_stats.stop_motifs_selected);
        }
        
        Ok(())
    }
    
    /// Build codebase info from extraction context for pattern mining
    fn build_codebase_info(&self, context: &ExtractionContext) -> Result<CodebaseInfo> {
        let mut functions = Vec::new();
        let mut file_info = HashMap::new();
        let mut total_lines = 0;
        
        for (entity_id, entity) in &context.entity_index {
            // Convert entities to functions for pattern mining
            functions.push(FunctionInfo {
                id: entity_id.clone(),
                source_code: entity.source_code.clone(),
                file_path: if entity.file_path.is_empty() { "unknown".to_string() } else { entity.file_path.clone() },
                line_count: entity.source_code.lines().count(),
            });
            
            total_lines += entity.source_code.lines().count();
            
            // Collect file-level info
            if !entity.file_path.is_empty() {
                let file_path = &entity.file_path;
                file_info.entry(file_path.clone())
                    .or_insert_with(|| FileInfo {
                        line_count: entity.source_code.lines().count(),
                        content_hash: sha2::Sha256::digest(entity.source_code.as_bytes()).to_vec(),
                    });
            }
        }
        
        Ok(CodebaseInfo {
            functions,
            total_lines,
            file_info,
        })
    }
    
    fn initialize_features(&mut self) {
        self.features = vec![
            FeatureDefinition::new(
                "saved_tokens_score",
                "Potential token savings from deduplication"
            )
            .with_range(0.0, 10000.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "rarity_gain",
                "Rarity gain from matched structural motifs"
            )
            .with_range(0.0, 10.0)
            .with_default(1.0),
            
            FeatureDefinition::new(
                "structural_evidence",
                "Structural evidence score (basic blocks + motifs)"
            )
            .with_range(0.0, 1.0)
            .with_default(0.0),
            
            FeatureDefinition::new(
                "live_reach_boost",
                "Live reachability boost factor"
            )
            .with_range(1.0, 10.0)
            .with_default(1.0),
            
            FeatureDefinition::new(
                "final_clone_score",
                "Final comprehensive clone detection score"
            )
            .with_range(0.0, 100000.0)
            .with_default(0.0),
        ];
    }
}

/// Phase 2 Structural Gate Analyzer - Eliminates low-quality clone matches
#[derive(Debug)]
pub struct StructuralGateAnalyzer {
    /// Basic block analyzer for computing matched blocks
    basic_block_analyzer: BasicBlockAnalyzer,
    
    /// PDG motif analyzer for structural pattern analysis
    pdg_motif_analyzer: Arc<std::sync::Mutex<PdgMotifAnalyzer>>,
    
    /// Configuration for structural gates
    config: StructuralGateConfig,
}

impl StructuralGateAnalyzer {
    pub fn new(config: StructuralGateConfig) -> Self {
        Self {
            basic_block_analyzer: BasicBlockAnalyzer::new(),
            pdg_motif_analyzer: Arc::new(std::sync::Mutex::new(PdgMotifAnalyzer::new(config.wl_iterations))),
            config,
        }
    }
    
    /// Apply structural gates to filter clone candidates
    pub fn apply_structural_gates(&mut self, candidate: &CloneCandidate, 
                                  code1: &str, code2: &str) -> Option<FilteredCloneCandidate> {
        tracing::debug!("Applying structural gates to candidate: {} vs {}", 
                   candidate.entity_id, candidate.similar_entity_id);
        
        // Phase 2 Gate 1: Basic Block Analysis
        let blocks1 = self.basic_block_analyzer.analyze(code1);
        let blocks2 = self.basic_block_analyzer.analyze(code2);
        
        // Compute structural match info (simplified match regions for demo)
        let match_info = self.basic_block_analyzer.compute_matched_blocks(
            &blocks1, &blocks2,
            0, code1.lines().count(), // Full match regions for demo
            0, code2.lines().count()
        );
        
        let min_matched_blocks = match_info.matched_blocks_1.min(match_info.matched_blocks_2);
        
        // Gate 1: Require minimum matched blocks
        if min_matched_blocks < self.config.require_blocks {
            tracing::debug!("Rejected: insufficient matched blocks ({} < {})", 
                       min_matched_blocks, self.config.require_blocks);
            return None;
        }
        
        // Phase 2 Gate 2: PDG Motif Analysis  
        let motifs1 = {
            let mut analyzer = self.pdg_motif_analyzer.lock().unwrap();
            analyzer.extract_motifs(code1, &candidate.entity_id)
        };
        
        let motifs2 = {
            let mut analyzer = self.pdg_motif_analyzer.lock().unwrap();
            analyzer.extract_motifs(code2, &candidate.similar_entity_id)
        };
        
        let shared_motifs = self.count_shared_motifs(&motifs1, &motifs2);
        
        // Gate 2: Require minimum shared motifs
        if shared_motifs < self.config.min_shared_motifs {
            tracing::debug!("Rejected: insufficient shared motifs ({} < {})", 
                       shared_motifs, self.config.min_shared_motifs);
            return None;
        }
        
        // Phase 2 Gate 3: IO/Side-effects Penalty
        let mut similarity_score = candidate.score;
        
        if match_info.external_call_jaccard < self.config.external_call_jaccard_threshold {
            similarity_score *= self.config.io_penalty_multiplier;
            tracing::debug!("Applied IO penalty: external calls differ significantly (jaccard: {:.3})", 
                       match_info.external_call_jaccard);
        }
        
        tracing::debug!("Structural gates passed: blocks={}, motifs={}, score={:.3}", 
                   min_matched_blocks, shared_motifs, similarity_score);
        
        Some(FilteredCloneCandidate {
            original: candidate.clone(),
            adjusted_score: similarity_score,
            structural_info: match_info,
            shared_motifs,
            motif_details: MotifAnalysisDetails {
                motifs_1: motifs1.len(),
                motifs_2: motifs2.len(),
                shared_branch_motifs: self.count_shared_motifs_by_category(&motifs1, &motifs2, MotifCategory::Branch),
                shared_loop_motifs: self.count_shared_motifs_by_category(&motifs1, &motifs2, MotifCategory::Loop),
                shared_call_motifs: self.count_shared_motifs_by_category(&motifs1, &motifs2, MotifCategory::Call),
            },
        })
    }
    
    /// Count shared motifs between two motif sets based on WL hash
    fn count_shared_motifs(&self, motifs1: &[PdgMotif], motifs2: &[PdgMotif]) -> usize {
        let hash_set1: HashSet<&String> = motifs1.iter().map(|m| &m.wl_hash).collect();
        let hash_set2: HashSet<&String> = motifs2.iter().map(|m| &m.wl_hash).collect();
        
        hash_set1.intersection(&hash_set2).count()
    }
    
    /// Count shared motifs by category
    fn count_shared_motifs_by_category(&self, motifs1: &[PdgMotif], motifs2: &[PdgMotif], 
                                       category: MotifCategory) -> usize {
        let filtered1: Vec<_> = motifs1.iter().filter(|m| m.motif_category == category).collect();
        let filtered2: Vec<_> = motifs2.iter().filter(|m| m.motif_category == category).collect();
        
        self.count_shared_motifs(&filtered1.into_iter().cloned().collect::<Vec<_>>(), 
                                 &filtered2.into_iter().cloned().collect::<Vec<_>>())
    }
}

/// Configuration for structural gates
#[derive(Debug, Clone)]
pub struct StructuralGateConfig {
    /// Minimum number of matched basic blocks required (Phase 2 Gate 1)
    pub require_blocks: usize,
    
    /// Minimum number of shared PDG motifs required (Phase 2 Gate 2)  
    pub min_shared_motifs: usize,
    
    /// External call Jaccard threshold for IO penalty (Phase 2 Gate 3)
    pub external_call_jaccard_threshold: f64,
    
    /// IO penalty multiplier when external calls differ
    pub io_penalty_multiplier: f64,
    
    /// Weisfeiler-Lehman iterations for motif hashing
    pub wl_iterations: usize,
}

impl Default for StructuralGateConfig {
    fn default() -> Self {
        Self {
            require_blocks: 2,
            min_shared_motifs: 2,
            external_call_jaccard_threshold: 0.2,
            io_penalty_multiplier: 0.7,
            wl_iterations: 3,
        }
    }
}

/// Clone candidate that passed structural gates
#[derive(Debug, Clone)]
pub struct FilteredCloneCandidate {
    pub original: CloneCandidate,
    pub adjusted_score: f64,
    pub structural_info: StructuralMatchInfo,
    pub shared_motifs: usize,
    pub motif_details: MotifAnalysisDetails,
}

/// Detailed motif analysis information
#[derive(Debug, Clone)]
pub struct MotifAnalysisDetails {
    pub motifs_1: usize,
    pub motifs_2: usize,
    pub shared_branch_motifs: usize,
    pub shared_loop_motifs: usize,
    pub shared_call_motifs: usize,
}

/// Auto-calibration engine for adaptive threshold tuning
#[derive(Debug)]
pub struct AutoCalibrationEngine {
    /// Noise metrics calculator
    noise_metrics: NoiseMetrics,
    
    /// Threshold sweeping system
    threshold_sweeper: ThresholdSweeper,
    
    /// Quality assessor
    quality_assessor: QualityAssessor,
    
    /// Cache manager for persisting tuned thresholds
    cache_manager: CacheManager,
}

impl AutoCalibrationEngine {
    fn new() -> Self {
        Self {
            noise_metrics: NoiseMetrics::new(),
            threshold_sweeper: ThresholdSweeper::new(),
            quality_assessor: QualityAssessor::new(),
            cache_manager: CacheManager::new(),
        }
    }
    
    /// Run auto-calibration on top N candidates
    pub fn auto_calibrate(&mut self, candidates: &[CloneCandidate], target_quality: f64) -> CalibrationResult {
        // Sweep thresholds on top 200 candidates
        let top_candidates = self.get_top_candidates(candidates, 200);
        
        let mut best_thresholds = AdaptiveThresholds::default();
        let mut best_quality = 0.0;
        
        // Grid search over threshold space
        for fragmentarity_thresh in [0.1, 0.2, 0.3, 0.4, 0.5].iter() {
            for structure_ratio_thresh in [0.6, 0.7, 0.8].iter() {
                for uniqueness_thresh in [1.2, 1.5, 2.0].iter() {
                    let thresholds = AdaptiveThresholds {
                        fragmentarity_threshold: *fragmentarity_thresh,
                        structure_ratio_threshold: *structure_ratio_thresh,
                        uniqueness_threshold: *uniqueness_thresh,
                        ..Default::default()
                    };
                    
                    let quality = self.assess_quality(&top_candidates, &thresholds);
                    
                    if quality >= target_quality && quality > best_quality {
                        best_quality = quality;
                        best_thresholds = thresholds;
                    }
                }
            }
        }
        
        CalibrationResult {
            thresholds: best_thresholds,
            quality_score: best_quality,
            candidates_processed: top_candidates.len(),
        }
    }
    
    fn get_top_candidates(&self, candidates: &[CloneCandidate], n: usize) -> Vec<CloneCandidate> {
        let mut sorted = candidates.to_vec();
        sorted.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        sorted.into_iter().take(n).collect()
    }
    
    fn assess_quality(&self, candidates: &[CloneCandidate], thresholds: &AdaptiveThresholds) -> f64 {
        let mut quality_count = 0;
        
        for candidate in candidates {
            if self.meets_quality_criteria(candidate, thresholds) {
                quality_count += 1;
            }
        }
        
        quality_count as f64 / candidates.len() as f64
    }
    
    fn meets_quality_criteria(&self, candidate: &CloneCandidate, thresholds: &AdaptiveThresholds) -> bool {
        let fragmentarity = self.noise_metrics.calculate_fragmentarity(candidate);
        let structure_ratio = self.noise_metrics.calculate_structure_ratio(candidate);
        let uniqueness = self.noise_metrics.calculate_uniqueness(candidate);
        
        fragmentarity >= thresholds.fragmentarity_threshold &&
        structure_ratio >= thresholds.structure_ratio_threshold &&
        uniqueness >= thresholds.uniqueness_threshold
    }
}

/// Noise metrics for quality assessment
#[derive(Debug)]
pub struct NoiseMetrics {
}

impl NoiseMetrics {
    fn new() -> Self {
        Self {}
    }
    
    fn calculate_fragmentarity(&self, candidate: &CloneCandidate) -> f64 {
        // Measure how fragmented the clone is
        candidate.matched_blocks as f64 / candidate.total_blocks.max(1) as f64
    }
    
    fn calculate_structure_ratio(&self, candidate: &CloneCandidate) -> f64 {
        // Measure structural content vs simple token overlap
        candidate.structural_motifs as f64 / candidate.total_motifs.max(1) as f64
    }
    
    fn calculate_uniqueness(&self, candidate: &CloneCandidate) -> f64 {
        // Measure how unique/rare the patterns are
        candidate.rarity_gain
    }
}

/// Threshold sweeping system
#[derive(Debug)]
pub struct ThresholdSweeper {
}

impl ThresholdSweeper {
    fn new() -> Self {
        Self {}
    }
}

/// Quality assessor
#[derive(Debug)]
pub struct QualityAssessor {
}

impl QualityAssessor {
    fn new() -> Self {
        Self {}
    }
}

/// Cache manager for persisting calibration results
#[derive(Debug)]
pub struct CacheManager {
}

impl CacheManager {
    fn new() -> Self {
        Self {}
    }
}

/// Clone candidate for evaluation
#[derive(Debug, Clone)]
pub struct CloneCandidate {
    pub entity_id: String,
    pub similar_entity_id: String,
    pub score: f64,
    pub saved_tokens: usize,
    pub rarity_gain: f64,
    pub matched_blocks: usize,
    pub total_blocks: usize,
    pub structural_motifs: usize,
    pub total_motifs: usize,
    pub live_reach_boost: f64,
}

/// Adaptive thresholds for auto-calibration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveThresholds {
    pub fragmentarity_threshold: f64,
    pub structure_ratio_threshold: f64,
    pub uniqueness_threshold: f64,
    pub min_saved_tokens: usize,
    pub stop_motif_percentile: f64,
}

impl Default for AdaptiveThresholds {
    fn default() -> Self {
        Self {
            fragmentarity_threshold: 0.3,
            structure_ratio_threshold: 0.7,
            uniqueness_threshold: 1.2,
            min_saved_tokens: 100,
            stop_motif_percentile: 0.75,
        }
    }
}

/// Calibration result
#[derive(Debug)]
pub struct CalibrationResult {
    pub thresholds: AdaptiveThresholds,
    pub quality_score: f64,
    pub candidates_processed: usize,
}

/// Phase 2 filtering statistics for comprehensive analysis
#[derive(Debug, Default)]
pub struct Phase2FilteringStats {
    pub total_candidates_processed: usize,
    pub total_passed_structural_gates: usize,
    pub total_rejected_insufficient_blocks: usize,
    pub total_rejected_insufficient_motifs: usize,
    pub total_io_penalties_applied: usize,
    pub filtering_rounds: usize,
}

impl Phase2FilteringStats {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn update_filtering_round(&mut self, input_count: usize, output_count: usize, 
                                  rejected_stats: RejectionStats) {
        self.total_candidates_processed += input_count;
        self.total_passed_structural_gates += output_count;
        self.total_rejected_insufficient_blocks += rejected_stats.insufficient_blocks;
        self.total_rejected_insufficient_motifs += rejected_stats.insufficient_motifs;
        self.total_io_penalties_applied += rejected_stats.io_penalties_applied;
        self.filtering_rounds += 1;
    }
    
    pub fn log_comprehensive_stats(&self) {
        tracing::info!("=== Phase 2 Structural Gate Statistics ====");
        tracing::info!("Total candidates processed: {}", self.total_candidates_processed);
        tracing::info!("Passed structural gates: {}", self.total_passed_structural_gates);
        tracing::info!("Rejected (insufficient blocks): {}", self.total_rejected_insufficient_blocks);
        tracing::info!("Rejected (insufficient motifs): {}", self.total_rejected_insufficient_motifs);
        tracing::info!("IO penalties applied: {}", self.total_io_penalties_applied);
        
        if self.total_candidates_processed > 0 {
            let pass_rate = (self.total_passed_structural_gates as f64 / self.total_candidates_processed as f64) * 100.0;
            tracing::info!("Overall pass rate: {:.1}%", pass_rate);
            
            let block_rejection_rate = (self.total_rejected_insufficient_blocks as f64 / self.total_candidates_processed as f64) * 100.0;
            let motif_rejection_rate = (self.total_rejected_insufficient_motifs as f64 / self.total_candidates_processed as f64) * 100.0;
            
            tracing::info!("Block rejection rate: {:.1}%", block_rejection_rate);  
            tracing::info!("Motif rejection rate: {:.1}%", motif_rejection_rate);
        }
        tracing::info!("===========================================");
    }
}

/// Statistics for tracking rejection reasons
#[derive(Debug, Default)]
pub struct RejectionStats {
    pub insufficient_blocks: usize,
    pub insufficient_motifs: usize,
    pub io_penalties_applied: usize,
}

impl RejectionStats {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn increment_structural_rejection(&mut self) {
        // This is a simplified version - in practice, we'd track specific rejection reasons
        self.insufficient_blocks += 1;
    }
}

#[async_trait]
impl FeatureExtractor for ComprehensiveCloneDetector {
    fn name(&self) -> &str {
        "comprehensive_clone_detection"
    }
    
    fn features(&self) -> &[FeatureDefinition] {
        &self.features
    }
    
    async fn extract(
        &self,
        entity: &CodeEntity,
        context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Extract and normalize tokens
        let tokens = self.extract_normalized_tokens(&entity.source_code)?;
        
        // Generate TF-IDF weighted similarity
        let tfidf_scores = {
            let mut analyzer = self.tfidf_analyzer.lock().unwrap();
            analyzer.add_document(entity.id.clone(), tokens.clone());
            analyzer.get_tfidf_vector(&entity.id)
        };
        
        // Extract PDG motifs
        let motifs = {
            let mut pdg = self.pdg_analyzer.lock().unwrap();
            pdg.extract_motifs(&entity.source_code, &entity.id)
        };
        
        // Validate structural evidence
        let structural_evidence = self.validate_structural_evidence(&motifs)?;
        
        // Calculate ranking scores
        let saved_tokens = self.calculate_saved_tokens(entity, context).await?;
        let rarity_gain = {
            let mut pdg = self.pdg_analyzer.lock().unwrap();
            pdg.calculate_rarity_gain(&motifs)
        };
        let live_reach_boost = self.calculate_live_reach_boost(entity)?;
        
        // Calculate final score
        let final_score = saved_tokens as f64 * rarity_gain * live_reach_boost;
        
        features.insert("saved_tokens_score".to_string(), saved_tokens as f64);
        features.insert("rarity_gain".to_string(), rarity_gain);
        features.insert("structural_evidence".to_string(), structural_evidence);
        features.insert("live_reach_boost".to_string(), live_reach_boost);
        features.insert("final_clone_score".to_string(), final_score);
        
        Ok(features)
    }
    
    fn supports_entity(&self, _entity: &CodeEntity) -> bool {
        true // Supports all entities
    }
}

impl ComprehensiveCloneDetector {
    /// Extract and normalize tokens from source code
    fn extract_normalized_tokens(&self, source_code: &str) -> Result<Vec<String>> {
        let mut tokens = Vec::new();
        
        for line in source_code.lines() {
            let line = line.trim();
            
            // Skip comments if configured
            if line.starts_with("//") || line.starts_with('#') {
                continue;
            }
            
            // Skip imports if configured
            if line.starts_with("import ") || line.starts_with("from ") || line.starts_with("use ") {
                continue;
            }
            
            // Tokenize and normalize
            let line_tokens: Vec<String> = line
                .split_whitespace()
                .filter(|token| !token.is_empty())
                .map(|token| self.normalize_token(token))
                .collect();
            
            tokens.extend(line_tokens);
        }
        
        Ok(tokens)
    }
    
    /// Normalize a single token
    fn normalize_token(&self, token: &str) -> String {
        // Apply alpha-rename for locals (simplified)
        if token.len() < 20 && token.chars().all(|c| c.is_alphanumeric() || c == '_') {
            if token.chars().any(|c| c.is_lowercase()) {
                return "LOCAL_VAR".to_string();
            }
        }
        
        // Bucket literals
        if token.parse::<f64>().is_ok() {
            return if token.contains('.') { "FLOAT_LIT" } else { "INT_LIT" }.to_string();
        }
        
        if (token.starts_with('"') && token.ends_with('"')) ||
           (token.starts_with('\'') && token.ends_with('\'')) {
            return "STR_LIT".to_string();
        }
        
        token.to_string()
    }
    
    /// Validate structural evidence requirements
    fn validate_structural_evidence(&self, motifs: &[PdgMotif]) -> Result<f64> {
        // Require matched regions span ≥2 basic blocks
        let control_flow_motifs = motifs.iter()
            .filter(|m| m.motif_type == MotifType::ControlFlow)
            .count();
        
        // Require shared ≥2 PDG motifs
        let total_motifs = motifs.len();
        
        if control_flow_motifs >= 2 && total_motifs >= 2 {
            Ok(1.0) // Full structural evidence
        } else if control_flow_motifs >= 1 || total_motifs >= 1 {
            Ok(0.5) // Partial structural evidence
        } else {
            Ok(0.0) // No structural evidence
        }
    }
    
    /// Calculate saved tokens from potential deduplication
    async fn calculate_saved_tokens(&self, entity: &CodeEntity, context: &ExtractionContext) -> Result<usize> {
        let entity_tokens = self.count_tokens(&entity.source_code);
        
        // Find similar entities in context
        let mut max_saved = 0;
        
        for (other_id, other_entity) in &context.entity_index {
            if other_id == &entity.id {
                continue;
            }
            
            let other_tokens = self.count_tokens(&other_entity.source_code);
            let union_tokens = self.estimate_union_tokens(&entity.source_code, &other_entity.source_code);
            
            let saved = entity_tokens + other_tokens - union_tokens;
            max_saved = max_saved.max(saved);
        }
        
        Ok(max_saved)
    }
    
    /// Count tokens in source code
    fn count_tokens(&self, source_code: &str) -> usize {
        source_code
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .count()
    }
    
    /// Estimate union size for token savings calculation
    fn estimate_union_tokens(&self, code1: &str, code2: &str) -> usize {
        // Simplified union estimation - in practice would use more sophisticated analysis
        let tokens1: HashSet<String> = code1.split_whitespace().map(String::from).collect();
        let tokens2: HashSet<String> = code2.split_whitespace().map(String::from).collect();
        
        tokens1.union(&tokens2).count()
    }
    
    /// Calculate live reachability boost
    fn calculate_live_reach_boost(&self, _entity: &CodeEntity) -> Result<f64> {
        // Placeholder - would integrate with actual live reachability data
        Ok(1.0 + 0.5) // 1.5x boost for demo
    }
}

/// Phase 4: Payoff Ranking System for intelligent clone prioritization
#[derive(Debug, Clone)]
pub struct PayoffRankingSystem {
    /// Live reach data when available
    live_reach_data: Option<HashMap<String, f64>>,
    
    /// IDF statistics for rarity calculations
    idf_statistics: IdfStatistics,
    
    /// Hard filtering floors
    hard_floors: HardFilteringFloors,
}

impl PayoffRankingSystem {
    pub fn new() -> Self {
        Self {
            live_reach_data: None,
            idf_statistics: IdfStatistics::new(),
            hard_floors: HardFilteringFloors::default(),
        }
    }
    
    pub fn with_live_reach_data(mut self, data: HashMap<String, f64>) -> Self {
        self.live_reach_data = Some(data);
        self
    }
    
    pub fn with_idf_statistics(mut self, stats: IdfStatistics) -> Self {
        self.idf_statistics = stats;
        self
    }
    
    /// Apply payoff ranking with the complete formula
    pub fn rank_candidates(&self, candidates: Vec<CloneCandidate>) -> Vec<RankedCloneCandidate> {
        let mut ranked_candidates: Vec<RankedCloneCandidate> = candidates
            .into_iter()
            .filter_map(|candidate| {
                // Apply hard filtering floors first
                if !self.passes_hard_floors(&candidate) {
                    return None;
                }
                
                let payoff_score = self.calculate_payoff_score(&candidate);
                
                Some(RankedCloneCandidate {
                    candidate,
                    payoff_score,
                    rank: 0, // Will be set after sorting
                })
            })
            .collect();
        
        // Sort by payoff score (descending)
        ranked_candidates.sort_by(|a, b| {
            b.payoff_score.partial_cmp(&a.payoff_score).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Assign ranks
        for (i, candidate) in ranked_candidates.iter_mut().enumerate() {
            candidate.rank = i + 1;
        }
        
        ranked_candidates
    }
    
    /// Calculate payoff score using the complete ranking formula
    fn calculate_payoff_score(&self, candidate: &CloneCandidate) -> f64 {
        let saved_tokens = candidate.saved_tokens as f64;
        let rarity_gain = candidate.rarity_gain;
        let live_reach_boost = self.get_live_reach_boost(&candidate.entity_id);
        let similarity_max = candidate.score; // Using similarity as max score
        
        // Complete ranking formula: similarity_max * saved_tokens * rarity_gain * live_reach_boost
        similarity_max * saved_tokens * rarity_gain * live_reach_boost
    }
    
    /// Get live reach boost for an entity
    fn get_live_reach_boost(&self, entity_id: &str) -> f64 {
        match &self.live_reach_data {
            Some(data) => {
                let median_reach = data.get(entity_id).copied().unwrap_or(0.0);
                1.0 + median_reach // Default 1.0 if no live data
            }
            None => 1.0, // No live data available
        }
    }
    
    /// Check if candidate passes hard filtering floors
    fn passes_hard_floors(&self, candidate: &CloneCandidate) -> bool {
        candidate.saved_tokens >= self.hard_floors.min_saved_tokens &&
        candidate.rarity_gain >= self.hard_floors.min_rarity_gain
    }
}

/// IDF statistics for rarity calculations
#[derive(Debug, Clone)]
pub struct IdfStatistics {
    pub repo_median_idf: f64,
    pub mean_idf_overall: f64,
    pub term_idf_scores: HashMap<String, f64>,
}

impl IdfStatistics {
    pub fn new() -> Self {
        Self {
            repo_median_idf: 1.0,
            mean_idf_overall: 1.0,
            term_idf_scores: HashMap::new(),
        }
    }
    
    pub fn calculate_mean_idf_matched(&self, matched_terms: &[String]) -> f64 {
        if matched_terms.is_empty() {
            return 0.0;
        }
        
        let sum: f64 = matched_terms
            .iter()
            .map(|term| self.term_idf_scores.get(term).copied().unwrap_or(1.0))
            .sum();
        
        sum / matched_terms.len() as f64
    }
}

/// Hard filtering floors for noise reduction
#[derive(Debug, Clone)]
pub struct HardFilteringFloors {
    pub min_saved_tokens: usize,
    pub min_rarity_gain: f64,
}

impl Default for HardFilteringFloors {
    fn default() -> Self {
        Self {
            min_saved_tokens: 100,
            min_rarity_gain: 1.2,
        }
    }
}

/// Ranked clone candidate with payoff score
#[derive(Debug, Clone)]
pub struct RankedCloneCandidate {
    pub candidate: CloneCandidate,
    pub payoff_score: f64,
    pub rank: usize,
}

/// Enhanced Auto-calibration engine with comprehensive threshold sweeping
impl AutoCalibrationEngine {
    /// Enhanced auto-calibration with binary search and quality diagnostics
    pub fn auto_calibrate_enhanced(&mut self, candidates: &[CloneCandidate], 
                                   target_quality: f64) -> Result<CalibrationResult> {
        tracing::info!("Starting enhanced auto-calibration with {} candidates", candidates.len());
        
        // Sample top 200 raw candidates before filtering
        let top_candidates = self.get_top_candidates(candidates, 200);
        
        // Perform binary search sweep on key thresholds
        let calibrated_thresholds = self.binary_search_thresholds(&top_candidates, target_quality)?;
        
        let quality_percentage = self.assess_quality(&top_candidates, &calibrated_thresholds);
        
        let calibration_result = CalibrationResult {
            thresholds: calibrated_thresholds.clone(),
            quality_score: quality_percentage,
            candidates_processed: top_candidates.len(),
        };
        
        // Persist calibration results to cache
        self.cache_manager.persist_calibration(&calibration_result)?;
        
        tracing::info!("Auto-calibration complete: {:.1}% quality achieved with {} candidates", 
                      quality_percentage * 100.0, top_candidates.len());
        
        Ok(calibration_result)
    }
    
    /// Binary search sweep of thresholds
    fn binary_search_thresholds(&self, candidates: &[CloneCandidate], 
                                target_quality: f64) -> Result<AdaptiveThresholds> {
        let mut best_thresholds = AdaptiveThresholds::default();
        let mut best_quality = 0.0;
        
        // Binary search on min_match_tokens (±8 range)
        for min_tokens in [50, 75, 100, 125, 150, 175, 200] {
            // Binary search on similarity threshold (±0.02 range) 
            for similarity in [0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9] {
                // Search require_blocks (1→2→3)
                for blocks in [1, 2, 3] {
                    let thresholds = AdaptiveThresholds {
                        fragmentarity_threshold: 0.4,
                        structure_ratio_threshold: 0.4,
                        uniqueness_threshold: 1.0, // Will use repo median
                        min_saved_tokens: min_tokens,
                        stop_motif_percentile: similarity,
                        ..Default::default()
                    };
                    
                    let quality = self.assess_quality_enhanced(candidates, &thresholds);
                    
                    if quality >= target_quality && quality > best_quality {
                        best_quality = quality;
                        best_thresholds = thresholds;
                    }
                }
            }
        }
        
        // If no threshold combination meets target, return best found
        if best_quality == 0.0 {
            tracing::warn!("No threshold combination achieved target quality {:.1}%, using defaults", 
                          target_quality * 100.0);
            return Ok(AdaptiveThresholds::default());
        }
        
        Ok(best_thresholds)
    }
    
    /// Enhanced quality assessment with comprehensive metrics
    fn assess_quality_enhanced(&self, candidates: &[CloneCandidate], 
                              thresholds: &AdaptiveThresholds) -> f64 {
        if candidates.is_empty() {
            return 0.0;
        }
        
        let mut quality_count = 0;
        
        for candidate in candidates {
            let quality_metrics = self.calculate_quality_metrics(candidate);
            
            if quality_metrics.meets_all_targets(thresholds) {
                quality_count += 1;
            }
        }
        
        quality_count as f64 / candidates.len() as f64
    }
    
    /// Calculate comprehensive quality metrics
    fn calculate_quality_metrics(&self, candidate: &CloneCandidate) -> QualityMetrics {
        let min_func_tokens = candidate.total_blocks.max(1);
        let matched_tokens = candidate.matched_blocks;
        let matched_blocks = candidate.matched_blocks;
        let min_blocks = candidate.total_blocks.max(1);
        
        // Quality metrics as specified
        let fragmentarity = matched_tokens as f64 / min_func_tokens as f64;
        let structure_ratio = matched_blocks as f64 / min_blocks as f64;
        let uniqueness = candidate.rarity_gain; // Mean IDF of matched n-grams
        
        QualityMetrics {
            fragmentarity,
            structure_ratio,
            uniqueness,
        }
    }
}

/// Quality metrics for candidate assessment
#[derive(Debug, Clone)]
pub struct QualityMetrics {
    pub fragmentarity: f64,
    pub structure_ratio: f64,
    pub uniqueness: f64,
}

impl QualityMetrics {
    /// Check if all quality targets are met
    pub fn meets_all_targets(&self, thresholds: &AdaptiveThresholds) -> bool {
        self.fragmentarity >= thresholds.fragmentarity_threshold &&
        self.structure_ratio >= thresholds.structure_ratio_threshold &&
        self.uniqueness >= thresholds.uniqueness_threshold
    }
}

/// Enhanced cache manager with persistent storage
impl CacheManager {
    /// Persist calibration results to cache directory
    pub fn persist_calibration(&self, result: &CalibrationResult) -> Result<()> {
        use std::fs;
        use std::time::SystemTime;
        
        // Create cache directory structure
        let cache_dir = std::path::Path::new(".valknut/cache/denoise");
        fs::create_dir_all(cache_dir).map_err(|e| {
            ValknutError::Io { 
                message: format!("Failed to create cache directory: {}", e),
                source: e
            }
        })?;
        
        // Create calibration entry with timestamp
        let calibration_entry = CachedCalibration {
            thresholds: result.thresholds.clone(),
            quality_percentage: result.quality_score * 100.0,
            candidates_processed: result.candidates_processed,
            timestamp: SystemTime::now(),
        };
        
        // Write to cache file
        let cache_file = cache_dir.join("auto_calibration.v1.json");
        let json = serde_json::to_string_pretty(&calibration_entry).map_err(|e| {
            ValknutError::Serialization {
                message: format!("Failed to serialize calibration: {}", e),
                data_type: Some("CachedCalibration".to_string()),
                source: Some(Box::new(e)),
            }
        })?;
        
        fs::write(&cache_file, json).map_err(|e| {
            ValknutError::Io {
                message: format!("Failed to write calibration cache: {}", e),
                source: e
            }
        })?;
        
        tracing::info!("Persisted calibration results to: {}", cache_file.display());
        Ok(())
    }
    
    /// Load cached calibration if available and fresh
    pub fn load_cached_calibration(&self) -> Result<Option<CalibrationResult>> {
        let cache_file = std::path::Path::new(".valknut/cache/denoise/auto_calibration.v1.json");
        
        if !cache_file.exists() {
            return Ok(None);
        }
        
        let json = std::fs::read_to_string(cache_file).map_err(|e| {
            ValknutError::Io {
                message: format!("Failed to read calibration cache: {}", e),
                source: e
            }
        })?;
        
        let cached: CachedCalibration = serde_json::from_str(&json).map_err(|e| {
            ValknutError::Serialization {
                message: format!("Failed to deserialize calibration: {}", e),
                data_type: Some("CachedCalibration".to_string()),
                source: Some(Box::new(e)),
            }
        })?;
        
        // Check if cache is fresh (less than 24 hours old)
        let age = SystemTime::now().duration_since(cached.timestamp)
            .unwrap_or(std::time::Duration::from_secs(86400 * 2)); // Default to stale
        
        if age.as_secs() > 86400 { // 24 hours
            tracing::info!("Calibration cache is stale ({} hours old), will recalibrate", 
                          age.as_secs() / 3600);
            return Ok(None);
        }
        
        tracing::info!("Using cached calibration from {} hours ago", age.as_secs() / 3600);
        
        Ok(Some(CalibrationResult {
            thresholds: cached.thresholds,
            quality_score: cached.quality_percentage / 100.0,
            candidates_processed: cached.candidates_processed,
        }))
    }
}

/// Cached calibration entry for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCalibration {
    pub thresholds: AdaptiveThresholds,
    pub quality_percentage: f64,
    pub candidates_processed: usize,
    pub timestamp: SystemTime,
}

/// Enhanced DedupeConfig to support the new adaptive thresholds
impl AdaptiveThresholds {
    /// Apply thresholds to filter candidates
    pub fn apply_filtering(&self, candidates: Vec<CloneCandidate>) -> Vec<CloneCandidate> {
        candidates.into_iter()
            .filter(|candidate| {
                candidate.saved_tokens >= self.min_saved_tokens &&
                candidate.rarity_gain >= self.uniqueness_threshold
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tfidf_analyzer() {
        let mut analyzer = TfIdfAnalyzer::new(NormalizationConfig::default());
        
        analyzer.add_document("doc1".to_string(), vec!["hello".to_string(), "world".to_string()]);
        analyzer.add_document("doc2".to_string(), vec!["hello".to_string(), "rust".to_string()]);
        
        let tfidf_hello = analyzer.tf_idf("doc1", "hello");
        let tfidf_world = analyzer.tf_idf("doc1", "world");
        
        assert!(tfidf_hello > 0.0);
        assert!(tfidf_world > tfidf_hello); // "world" should be more unique
    }
    
    #[test]
    fn test_pdg_motif_analyzer() {
        let mut analyzer = PdgMotifAnalyzer::new(3);
        
        let code = r#"
            if x > 0 {
                for i in 0..10 {
                    println!("test");
                }
            }
        "#;
        
        let motifs = analyzer.extract_motifs(code, "test_entity");
        assert!(!motifs.is_empty());
        
        // Check that motifs have appropriate categories
        let has_branch = motifs.iter().any(|m| m.motif_category == MotifCategory::Branch);
        let has_loop = motifs.iter().any(|m| m.motif_category == MotifCategory::Loop);
        assert!(has_branch);
        assert!(has_loop);
    }
    
    #[test]
    fn test_weighted_minhash() {
        let mut weights = HashMap::new();
        weights.insert("important".to_string(), 2.0);
        weights.insert("common".to_string(), 0.5);
        
        let minhash = WeightedMinHash::new(64, weights);
        
        let tokens1 = vec!["important".to_string(), "common".to_string()];
        let tokens2 = vec!["important".to_string(), "other".to_string()];
        
        let sig1 = minhash.generate_signature(&tokens1);
        let sig2 = minhash.generate_signature(&tokens2);
        
        let similarity = sig1.jaccard_similarity(&sig2);
        assert!(similarity >= 0.0 && similarity <= 1.0);
    }
    
    #[test]
    fn test_basic_block_analyzer() {
        let mut analyzer = BasicBlockAnalyzer::new();
        
        let code = r#"
            fn test() {
                if x > 0 {
                    println!("positive");
                }
                for i in 0..10 {
                    process(i);
                }
            }
        "#;
        
        let blocks = analyzer.analyze(code);
        assert!(blocks.len() >= 2); // At least conditional and loop blocks
        
        // Verify blocks have line ranges
        for block in &blocks {
            if !block.lines.is_empty() {
                assert!(!block.line_ranges.is_empty());
                assert_eq!(block.lines.len(), block.line_ranges.len());
            }
        }
    }
    
    #[test]
    fn test_phase2_structural_gates_basic_blocks() {
        let config = StructuralGateConfig {
            require_blocks: 2,
            min_shared_motifs: 1,
            external_call_jaccard_threshold: 0.2,
            io_penalty_multiplier: 0.7,
            wl_iterations: 3,
        };
        
        let mut gate_analyzer = StructuralGateAnalyzer::new(config);
        
        // Test case 1: Code with sufficient basic blocks (should pass)
        let code_with_blocks = r#"
            fn complex_function() {
                if condition {
                    do_something();
                }
                for i in 0..10 {
                    process(i);
                }
                while running {
                    update();
                }
            }
        "#;
        
        // Test case 2: Simple code with insufficient blocks (should fail)
        let simple_code = r#"
            fn simple_function() {
                let x = 42;
            }
        "#;
        
        let candidate_pass = CloneCandidate {
            entity_id: "entity1".to_string(),
            similar_entity_id: "entity2".to_string(),
            score: 0.85,
            saved_tokens: 100,
            rarity_gain: 1.5,
            matched_blocks: 3,
            total_blocks: 4,
            structural_motifs: 2,
            total_motifs: 3,
            live_reach_boost: 1.2,
        };
        
        let candidate_fail = CloneCandidate {
            entity_id: "entity3".to_string(),
            similar_entity_id: "entity4".to_string(),
            score: 0.85,
            saved_tokens: 50,
            rarity_gain: 1.1,
            matched_blocks: 1, 
            total_blocks: 2,
            structural_motifs: 1,
            total_motifs: 2,
            live_reach_boost: 1.0,
        };
        
        // Should pass structural gates
        let result_pass = gate_analyzer.apply_structural_gates(&candidate_pass, code_with_blocks, code_with_blocks);
        assert!(result_pass.is_some());
        
        // Should fail structural gates  
        let result_fail = gate_analyzer.apply_structural_gates(&candidate_fail, simple_code, simple_code);
        assert!(result_fail.is_none());
    }
    
    #[test]
    fn test_phase2_motif_analysis() {
        let mut analyzer = PdgMotifAnalyzer::new(3);
        
        let code_with_motifs = r#"
            fn analyze_data(data: &[Item]) -> Result<Analysis> {
                if data.is_empty() {
                    return Err("No data");
                }
                
                let mut results = Vec::new();
                for item in data {
                    let processed = process_item(item)?;
                    results.push(processed);
                }
                
                let summary = calculate_summary(&results);
                Ok(Analysis { results, summary })
            }
        "#;
        
        let code_similar_structure = r#"
            fn process_items(items: &[Data]) -> Result<ProcessedData> {
                if items.is_empty() {
                    return Err("Empty input");
                }
                
                let mut output = Vec::new();
                for data_item in items {
                    let transformed = transform_item(data_item)?;
                    output.push(transformed);
                }
                
                let aggregate = compute_aggregate(&output);
                Ok(ProcessedData { output, aggregate })
            }
        "#;
        
        let motifs1 = analyzer.extract_motifs(code_with_motifs, "entity1");
        let motifs2 = analyzer.extract_motifs(code_similar_structure, "entity2");
        
        // Both should have motifs
        assert!(!motifs1.is_empty());
        assert!(!motifs2.is_empty());
        
        // Should have shared structural patterns (conditional + loop)
        let config = StructuralGateConfig::default();
        let gate_analyzer = StructuralGateAnalyzer::new(config);
        
        let shared_count = gate_analyzer.count_shared_motifs(&motifs1, &motifs2);
        assert!(shared_count >= 1, "Should have at least 1 shared motif");
    }
    
    #[test]
    fn test_phase2_external_call_analysis() {
        let mut analyzer = BasicBlockAnalyzer::new();
        
        let code_with_calls = r#"
            fn network_operations() {
                let client = HttpClient::new();
                let response = client.get("http://api.example.com").send()?;
                let data = response.json()?;
                database.save(data)?;
                logger.info("Saved data");
            }
        "#;
        
        let code_different_calls = r#"
            fn file_operations() {
                let file = File::open("data.txt")?;
                let content = io::read_to_string(file)?;
                let parsed = parse_content(content)?;
                cache.store(parsed)?;
            }
        "#;
        
        let blocks1 = analyzer.analyze(code_with_calls);
        let blocks2 = analyzer.analyze(code_different_calls);
        
        // Compute match info for different external calls
        let match_info = analyzer.compute_matched_blocks(
            &blocks1, &blocks2,
            0, code_with_calls.lines().count(),
            0, code_different_calls.lines().count()
        );
        
        // External call Jaccard should be low (different call patterns)
        assert!(match_info.external_call_jaccard < 0.5);
        assert!(match_info.total_external_calls_1 > 0);
        assert!(match_info.total_external_calls_2 > 0);
    }
    
    #[test]
    fn test_phase2_comprehensive_filtering() {
        let config = DedupeConfig::default();
        let mut detector = ComprehensiveCloneDetector::new(config);
        
        let candidates = vec![
            // Good candidate: sufficient blocks and motifs
            CloneCandidate {
                entity_id: "good1".to_string(),
                similar_entity_id: "good2".to_string(),
                score: 0.9,
                saved_tokens: 200,
                rarity_gain: 2.0,
                matched_blocks: 3,
                total_blocks: 4,
                structural_motifs: 3,
                total_motifs: 4,
                live_reach_boost: 1.5,
            },
            // Bad candidate: insufficient blocks
            CloneCandidate {
                entity_id: "bad1".to_string(),
                similar_entity_id: "bad2".to_string(),
                score: 0.8,
                saved_tokens: 50,
                rarity_gain: 1.0,
                matched_blocks: 1,
                total_blocks: 2,
                structural_motifs: 1,
                total_motifs: 2,
                live_reach_boost: 1.0,
            },
        ];
        
        let mut code_mapping = HashMap::new();
        code_mapping.insert("good1".to_string(), r#"
            fn complex_good() {
                if check_condition() {
                    for item in items {
                        process(item);
                    }
                }
                while running {
                    update_state();
                }
            }
        "#.to_string());
        
        code_mapping.insert("good2".to_string(), r#"
            fn complex_good_similar() {
                if validate_input() {
                    for element in elements {
                        handle(element);
                    }
                }
                while active {
                    refresh_data();
                }
            }
        "#.to_string());
        
        code_mapping.insert("bad1".to_string(), r#"
            fn simple_bad() {
                let x = 42;
            }
        "#.to_string());
        
        code_mapping.insert("bad2".to_string(), r#"
            fn simple_bad_similar() {
                let y = 24;
            }
        "#.to_string());
        
        let filtered = detector.filter_candidates_phase2(candidates, &code_mapping);
        
        // Only the good candidate should pass
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].original.entity_id, "good1");
        
        // Check statistics
        let stats = detector.get_phase2_stats();
        assert_eq!(stats.total_candidates_processed, 2);
        assert_eq!(stats.total_passed_structural_gates, 1);
    }
    
    #[test]  
    fn test_phase2_io_penalty_application() {
        let config = StructuralGateConfig {
            require_blocks: 1, // Low threshold to test penalty specifically
            min_shared_motifs: 1,
            external_call_jaccard_threshold: 0.2,
            io_penalty_multiplier: 0.7,
            wl_iterations: 3,
        };
        
        let mut gate_analyzer = StructuralGateAnalyzer::new(config);
        
        let code_network_io = r#"
            fn network_operations() {
                http_client.get("/api/data").send()?;
                database.query("SELECT * FROM table")?;
            }
        "#;
        
        let code_file_io = r#"
            fn file_operations() {
                file_system.read("data.txt")?;
                cache.store("key", value)?;
            }
        "#;
        
        let candidate = CloneCandidate {
            entity_id: "net1".to_string(),
            similar_entity_id: "file1".to_string(),
            score: 1.0, // Start with perfect score
            saved_tokens: 100,
            rarity_gain: 1.5,
            matched_blocks: 2,
            total_blocks: 2,
            structural_motifs: 2,
            total_motifs: 2,
            live_reach_boost: 1.0,
        };
        
        let result = gate_analyzer.apply_structural_gates(&candidate, code_network_io, code_file_io);
        assert!(result.is_some());
        
        let filtered = result.unwrap();
        // Score should be penalized due to different external calls  
        assert!(filtered.adjusted_score < candidate.score);
        assert!((filtered.adjusted_score - 0.7).abs() < 0.1); // Should be ~0.7 (1.0 * 0.7)
    }
    
    #[test]
    fn test_phase3_stop_motif_cache_integration() {
        use crate::io::cache::{StopMotifCache, StopMotifEntry, PatternCategory, MiningStats};
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // Create a mock stop-motifs cache
        let stop_motifs_cache = StopMotifCache {
            version: 1,
            k_gram_size: 9,
            token_grams: vec![
                StopMotifEntry {
                    pattern: "println!".to_string(),
                    support: 100,
                    idf_score: 1.5,
                    weight_multiplier: 0.2,
                    category: PatternCategory::Boilerplate,
                },
                StopMotifEntry {
                    pattern: "if LOCAL_VAR ==".to_string(),
                    support: 80,
                    idf_score: 1.8,
                    weight_multiplier: 0.2,
                    category: PatternCategory::TokenGram,
                },
            ],
            pdg_motifs: vec![
                StopMotifEntry {
                    pattern: "control:branch".to_string(),
                    support: 150,
                    idf_score: 2.2,
                    weight_multiplier: 0.2,
                    category: PatternCategory::ControlFlow,
                },
                StopMotifEntry {
                    pattern: "boiler:debug_print".to_string(),
                    support: 90,
                    idf_score: 1.9,
                    weight_multiplier: 0.2,
                    category: PatternCategory::Boilerplate,
                },
            ],
            ast_patterns: vec![], // Empty AST patterns for this test
            last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            codebase_signature: "test_signature".to_string(),
            mining_stats: MiningStats {
                functions_analyzed: 100,
                unique_kgrams_found: 500,
                unique_motifs_found: 200,
                ast_patterns_found: 50,
                ast_node_types_found: 25,
                ast_subtree_patterns_found: 15,
                stop_motifs_selected: 4,
                percentile_threshold: 0.5,
                mining_duration_ms: 1000,
                languages_processed: HashSet::from(["python".to_string(), "rust".to_string()]),
            },
        };
        
        // Test TF-IDF analyzer with stop-motifs
        let mut tfidf_analyzer = TfIdfAnalyzer::new(NormalizationConfig::default());
        tfidf_analyzer.set_stop_motif_cache(Arc::new(stop_motifs_cache.clone()));
        
        // Add documents
        tfidf_analyzer.add_document("doc1".to_string(), vec!["println!".to_string(), "test".to_string()]);
        tfidf_analyzer.add_document("doc2".to_string(), vec!["if".to_string(), "condition".to_string()]);
        
        // Test TF-IDF with stop-motifs adjustment
        let tfidf_println = tfidf_analyzer.tf_idf("doc1", "println!");
        let tfidf_test = tfidf_analyzer.tf_idf("doc1", "test");
        
        // println! should have lower score due to stop-motif adjustment
        assert!(tfidf_println < tfidf_test, "Stop-motif 'println!' should have lower TF-IDF score");
        
        // Test PDG motif analyzer with stop-motifs
        let mut pdg_analyzer = PdgMotifAnalyzer::new(3);
        pdg_analyzer.set_stop_motif_cache(Arc::new(stop_motifs_cache));
        
        let code_with_boilerplate = r#"
            fn test_function() {
                if condition {
                    println!("debug message");
                }
                for item in items {
                    process(item);
                }
            }
        "#;
        
        let motifs = pdg_analyzer.extract_motifs(code_with_boilerplate, "test_entity");
        let rarity_gain = pdg_analyzer.calculate_rarity_gain(&motifs);
        
        // Rarity gain should be calculated (exact value depends on implementation)
        assert!(rarity_gain > 0.0);
        
        // Test that motif extraction works
        assert!(!motifs.is_empty());
        let has_control_motifs = motifs.iter().any(|m| m.motif_category == MotifCategory::Branch || m.motif_category == MotifCategory::Loop);
        assert!(has_control_motifs);
    }
    
    #[test] 
    fn test_phase3_weight_application() {
        use crate::io::cache::{StopMotifCache, StopMotifEntry, PatternCategory, MiningStats};
        use std::time::{SystemTime, UNIX_EPOCH};
        
        // Create analyzer with stop-motifs
        let mut analyzer = TfIdfAnalyzer::new(NormalizationConfig::default());
        
        let stop_motifs_cache = StopMotifCache {
            version: 1,
            k_gram_size: 9,
            token_grams: vec![
                StopMotifEntry {
                    pattern: "common_pattern".to_string(),
                    support: 200,
                    idf_score: 1.0,
                    weight_multiplier: 0.2, // 20% weight
                    category: PatternCategory::Boilerplate,
                },
            ],
            pdg_motifs: vec![],
            ast_patterns: vec![], // Empty AST patterns for this test
            last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            codebase_signature: "test".to_string(),
            mining_stats: MiningStats::default(),
        };
        
        analyzer.set_stop_motif_cache(Arc::new(stop_motifs_cache));
        
        // Add test documents
        analyzer.add_document("doc1".to_string(), vec![
            "common_pattern".to_string(),
            "unique_token".to_string()
        ]);
        
        // Test weight application
        let score_common = analyzer.tf_idf("doc1", "common_pattern");
        let score_unique = analyzer.tf_idf("doc1", "unique_token");
        
        // Common pattern should have reduced weight (×0.2)
        assert!(score_common < score_unique);
        
        // Verify the multiplier is approximately correct
        // Note: exact comparison is tricky due to IDF calculations, 
        // but stop-motif should have significantly lower score
        assert!(score_common < score_unique * 0.5);
    }
    
    #[test]
    fn test_phase3_term_pattern_matching() {
        let analyzer = TfIdfAnalyzer::new(NormalizationConfig::default());
        
        // Test exact match
        assert!(analyzer.term_matches_pattern("println!", "println!"));
        assert!(!analyzer.term_matches_pattern("eprintln!", "println!"));
        
        // Test k-gram containment
        assert!(analyzer.term_matches_pattern("if x == 42", "x == 42"));
        assert!(analyzer.term_matches_pattern("x == 42", "if x == 42"));
        assert!(analyzer.term_matches_pattern("LOCAL_VAR == INT_LIT", "=="));
        
        // Test non-matching cases
        assert!(!analyzer.term_matches_pattern("different", "pattern"));
        assert!(!analyzer.term_matches_pattern("short", "much_longer_pattern"));
    }
    
    #[test]
    fn test_phase3_motif_pattern_matching() {
        use crate::io::cache::{StopMotifEntry, PatternCategory};
        
        let pdg_analyzer = PdgMotifAnalyzer::new(3);
        
        let test_motif = PdgMotif {
            motif_type: MotifType::ControlFlow,
            structure: "branch:if".to_string(),
            complexity: 1,
            wl_hash: "test_hash".to_string(),
            frequency: 1,
            motif_category: MotifCategory::Branch,
        };
        
        // Test category matching
        let stop_motif_branch = StopMotifEntry {
            pattern: "branch".to_string(),
            support: 100,
            idf_score: 1.0,
            weight_multiplier: 0.2,
            category: PatternCategory::ControlFlow,
        };
        
        assert!(pdg_analyzer.motif_matches_stop_pattern(&test_motif, &stop_motif_branch));
        
        // Test structure matching
        let stop_motif_structure = StopMotifEntry {
            pattern: "if".to_string(),
            support: 100,
            idf_score: 1.0,
            weight_multiplier: 0.2,
            category: PatternCategory::Assignment, // Different category
        };
        
        assert!(pdg_analyzer.motif_matches_stop_pattern(&test_motif, &stop_motif_structure));
        
        // Test non-matching case
        let stop_motif_no_match = StopMotifEntry {
            pattern: "loop".to_string(),
            support: 100,
            idf_score: 1.0,
            weight_multiplier: 0.2,
            category: PatternCategory::DataStructure,
        };
        
        assert!(!pdg_analyzer.motif_matches_stop_pattern(&test_motif, &stop_motif_no_match));
    }
    
    #[test]
    fn test_phase4_payoff_ranking_formula() {
        let payoff_ranking = PayoffRankingSystem::new();
        
        // Create test candidates with different characteristics
        let candidates = vec![
            // High-value candidate
            CloneCandidate {
                entity_id: "high_value".to_string(),
                similar_entity_id: "high_value_dup".to_string(),
                score: 0.9,                // High similarity
                saved_tokens: 500,         // High token savings
                rarity_gain: 2.5,         // High rarity
                matched_blocks: 8,
                total_blocks: 10,
                structural_motifs: 5,
                total_motifs: 6,
                live_reach_boost: 1.0,
            },
            // Low-value candidate (should be filtered by hard floors)
            CloneCandidate {
                entity_id: "low_value".to_string(),
                similar_entity_id: "low_value_dup".to_string(),
                score: 0.6,
                saved_tokens: 50,          // Below hard floor (100)
                rarity_gain: 1.0,         // Below hard floor (1.2)
                matched_blocks: 2,
                total_blocks: 3,
                structural_motifs: 1,
                total_motifs: 2,
                live_reach_boost: 1.0,
            },
            // Medium-value candidate
            CloneCandidate {
                entity_id: "medium_value".to_string(),
                similar_entity_id: "medium_value_dup".to_string(),
                score: 0.7,
                saved_tokens: 200,
                rarity_gain: 1.5,
                matched_blocks: 4,
                total_blocks: 5,
                structural_motifs: 3,
                total_motifs: 4,
                live_reach_boost: 1.0,
            },
        ];
        
        let ranked = payoff_ranking.rank_candidates(candidates);
        
        // Should only have 2 candidates after hard filtering
        assert_eq!(ranked.len(), 2);
        
        // High-value candidate should be ranked first
        assert_eq!(ranked[0].candidate.entity_id, "high_value");
        assert_eq!(ranked[0].rank, 1);
        
        // Medium-value candidate should be ranked second
        assert_eq!(ranked[1].candidate.entity_id, "medium_value");
        assert_eq!(ranked[1].rank, 2);
        
        // Verify payoff score calculation: similarity_max * saved_tokens * rarity_gain * live_reach_boost
        let expected_high_score = 0.9 * 500.0 * 2.5 * 1.0; // = 1125.0
        let expected_medium_score = 0.7 * 200.0 * 1.5 * 1.0; // = 210.0
        
        assert!((ranked[0].payoff_score - expected_high_score).abs() < 0.001);
        assert!((ranked[1].payoff_score - expected_medium_score).abs() < 0.001);
    }
    
    #[test]
    fn test_phase4_payoff_ranking_with_live_reach_data() {
        let mut live_reach_data = HashMap::new();
        live_reach_data.insert("high_reach".to_string(), 0.8); // 80% production reach
        live_reach_data.insert("low_reach".to_string(), 0.1);  // 10% production reach
        
        let payoff_ranking = PayoffRankingSystem::new()
            .with_live_reach_data(live_reach_data);
        
        let candidates = vec![
            // High reach candidate
            CloneCandidate {
                entity_id: "high_reach".to_string(),
                similar_entity_id: "high_reach_dup".to_string(),
                score: 0.8,
                saved_tokens: 150,
                rarity_gain: 1.3,
                matched_blocks: 3,
                total_blocks: 4,
                structural_motifs: 2,
                total_motifs: 3,
                live_reach_boost: 1.0, // Will be overridden
            },
            // Low reach candidate
            CloneCandidate {
                entity_id: "low_reach".to_string(),
                similar_entity_id: "low_reach_dup".to_string(),
                score: 0.85, // Slightly higher similarity
                saved_tokens: 150,
                rarity_gain: 1.3,
                matched_blocks: 3,
                total_blocks: 4,
                structural_motifs: 2,
                total_motifs: 3,
                live_reach_boost: 1.0, // Will be overridden
            },
        ];
        
        let ranked = payoff_ranking.rank_candidates(candidates);
        
        // High reach candidate should rank higher due to live_reach_boost
        assert_eq!(ranked[0].candidate.entity_id, "high_reach");
        
        // Verify live reach boost is applied: 1.0 + median_reach
        let high_reach_boost = 1.0 + 0.8; // 1.8
        let low_reach_boost = 1.0 + 0.1;  // 1.1
        
        let expected_high_score = 0.8 * 150.0 * 1.3 * high_reach_boost;
        let expected_low_score = 0.85 * 150.0 * 1.3 * low_reach_boost;
        
        assert!(expected_high_score > expected_low_score);
        assert!((ranked[0].payoff_score - expected_high_score).abs() < 0.001);
    }
    
    #[test]
    fn test_phase4_auto_calibration_quality_metrics() {
        let auto_calibration = AutoCalibrationEngine::new();
        
        // Create candidates with different quality characteristics
        let candidates = vec![
            // High quality candidate
            CloneCandidate {
                entity_id: "high_quality".to_string(),
                similar_entity_id: "high_quality_dup".to_string(),
                score: 0.9,
                saved_tokens: 300,
                rarity_gain: 2.0,     // High uniqueness
                matched_blocks: 8,    // Good structure ratio: 8/10 = 0.8
                total_blocks: 10,
                structural_motifs: 7,
                total_motifs: 8,
                live_reach_boost: 1.0,
            },
            // Low quality candidate
            CloneCandidate {
                entity_id: "low_quality".to_string(),
                similar_entity_id: "low_quality_dup".to_string(),
                score: 0.6,
                saved_tokens: 120,
                rarity_gain: 1.1,     // Low uniqueness
                matched_blocks: 2,    // Poor structure ratio: 2/10 = 0.2
                total_blocks: 10,
                structural_motifs: 1,
                total_motifs: 8,
                live_reach_boost: 1.0,
            },
        ];
        
        // Test quality metrics calculation
        let high_quality_metrics = auto_calibration.calculate_quality_metrics(&candidates[0]);
        let low_quality_metrics = auto_calibration.calculate_quality_metrics(&candidates[1]);
        
        // High quality candidate should have better metrics
        assert!(high_quality_metrics.fragmentarity > low_quality_metrics.fragmentarity);
        assert!(high_quality_metrics.structure_ratio > low_quality_metrics.structure_ratio);
        assert!(high_quality_metrics.uniqueness > low_quality_metrics.uniqueness);
        
        // Test quality threshold checking
        let strict_thresholds = AdaptiveThresholds {
            fragmentarity_threshold: 0.6,
            structure_ratio_threshold: 0.6,
            uniqueness_threshold: 1.8,
            min_saved_tokens: 200,
            stop_motif_percentile: 0.8,
        };
        
        assert!(high_quality_metrics.meets_all_targets(&strict_thresholds));
        assert!(!low_quality_metrics.meets_all_targets(&strict_thresholds));
        
        // Test lenient thresholds
        let lenient_thresholds = AdaptiveThresholds {
            fragmentarity_threshold: 0.1,
            structure_ratio_threshold: 0.1,
            uniqueness_threshold: 1.0,
            min_saved_tokens: 100,
            stop_motif_percentile: 0.5,
        };
        
        assert!(high_quality_metrics.meets_all_targets(&lenient_thresholds));
        assert!(low_quality_metrics.meets_all_targets(&lenient_thresholds));
    }
    
    #[test]
    fn test_phase4_hard_filtering_floors() {
        let payoff_ranking = PayoffRankingSystem::new();
        
        let candidates = vec![
            // Pass both floors
            CloneCandidate {
                entity_id: "pass_both".to_string(),
                similar_entity_id: "pass_both_dup".to_string(),
                score: 0.8,
                saved_tokens: 150,    // >= 100
                rarity_gain: 1.5,    // >= 1.2
                matched_blocks: 3,
                total_blocks: 4,
                structural_motifs: 2,
                total_motifs: 3,
                live_reach_boost: 1.0,
            },
            // Fail saved_tokens floor
            CloneCandidate {
                entity_id: "fail_tokens".to_string(),
                similar_entity_id: "fail_tokens_dup".to_string(),
                score: 0.8,
                saved_tokens: 50,     // < 100
                rarity_gain: 1.5,    // >= 1.2
                matched_blocks: 3,
                total_blocks: 4,
                structural_motifs: 2,
                total_motifs: 3,
                live_reach_boost: 1.0,
            },
            // Fail rarity_gain floor
            CloneCandidate {
                entity_id: "fail_rarity".to_string(),
                similar_entity_id: "fail_rarity_dup".to_string(),
                score: 0.8,
                saved_tokens: 150,    // >= 100
                rarity_gain: 1.0,    // < 1.2
                matched_blocks: 3,
                total_blocks: 4,
                structural_motifs: 2,
                total_motifs: 3,
                live_reach_boost: 1.0,
            },
            // Fail both floors
            CloneCandidate {
                entity_id: "fail_both".to_string(),
                similar_entity_id: "fail_both_dup".to_string(),
                score: 0.8,
                saved_tokens: 50,     // < 100
                rarity_gain: 1.0,    // < 1.2
                matched_blocks: 3,
                total_blocks: 4,
                structural_motifs: 2,
                total_motifs: 3,
                live_reach_boost: 1.0,
            },
        ];
        
        let ranked = payoff_ranking.rank_candidates(candidates);
        
        // Only the first candidate should pass the hard floors
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].candidate.entity_id, "pass_both");
    }
    
    #[test]
    fn test_phase4_idf_statistics() {
        let mut idf_stats = IdfStatistics::new();
        idf_stats.term_idf_scores.insert("rare_term".to_string(), 3.0);
        idf_stats.term_idf_scores.insert("common_term".to_string(), 1.0);
        idf_stats.term_idf_scores.insert("medium_term".to_string(), 2.0);
        
        // Test mean IDF calculation
        let rare_terms = vec!["rare_term".to_string(), "medium_term".to_string()];
        let common_terms = vec!["common_term".to_string(), "common_term".to_string()];
        let mixed_terms = vec!["rare_term".to_string(), "common_term".to_string(), "medium_term".to_string()];
        
        let rare_mean = idf_stats.calculate_mean_idf_matched(&rare_terms);
        let common_mean = idf_stats.calculate_mean_idf_matched(&common_terms);
        let mixed_mean = idf_stats.calculate_mean_idf_matched(&mixed_terms);
        
        assert!((rare_mean - 2.5).abs() < 0.001);   // (3.0 + 2.0) / 2
        assert!((common_mean - 1.0).abs() < 0.001); // (1.0 + 1.0) / 2
        assert!((mixed_mean - 2.0).abs() < 0.001);  // (3.0 + 1.0 + 2.0) / 3
        
        // Test empty terms
        let empty_mean = idf_stats.calculate_mean_idf_matched(&vec![]);
        assert_eq!(empty_mean, 0.0);
        
        // Test unknown terms (should default to 1.0)
        let unknown_terms = vec!["unknown_term".to_string()];
        let unknown_mean = idf_stats.calculate_mean_idf_matched(&unknown_terms);
        assert_eq!(unknown_mean, 1.0);
    }
}