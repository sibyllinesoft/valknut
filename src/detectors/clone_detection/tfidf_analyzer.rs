//! TF-IDF Analysis Engine for structure-aware clone detection

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::normalization::NormalizationConfig;

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
    pub fn new() -> Self {
        Self::new_with_config(NormalizationConfig::default())
    }

    /// Create a new TF-IDF analyzer with custom normalization config
    pub fn new_with_config(normalization_config: NormalizationConfig) -> Self {
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
    pub fn with_cache(mut self, cache: Arc<crate::io::cache::StopMotifCache>) -> Self {
        let token_grams_len = cache.token_grams.len();
        let pdg_motifs_len = cache.pdg_motifs.len();
        self.stop_motif_cache = Some(cache);
        tracing::info!(
            "Phase 3 stop-motifs cache enabled: {} token grams, {} PDG motifs",
            token_grams_len,
            pdg_motifs_len
        );
        self
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
        let tf = self
            .term_frequencies
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
    fn apply_stop_motif_adjustment(
        &self,
        term: &str,
        base_score: f64,
        cache: &crate::io::cache::StopMotifCache,
    ) -> f64 {
        // Check if term matches any stop-motif pattern
        for stop_motif in &cache.token_grams {
            if self.term_matches_pattern(term, &stop_motif.pattern) {
                let adjusted_score = base_score * stop_motif.weight_multiplier;
                tracing::trace!(
                    "Phase 3 stop-motif adjustment: '{}' -> {:.3} (×{:.1})",
                    term,
                    adjusted_score,
                    stop_motif.weight_multiplier
                );
                return adjusted_score;
            }
        }

        base_score
    }

    /// Check if a term matches a stop-motif pattern
    fn term_matches_pattern(&self, term: &str, pattern: &str) -> bool {
        // Check exact match first
        if term == pattern {
            return true;
        }

        // For multi-token patterns (contain spaces), check phrase containment
        if pattern.contains(' ') || term.contains(' ') {
            return term.contains(pattern) || pattern.contains(term);
        }

        // For single tokens, check word boundary matches
        // Split term into tokens and check if pattern matches any token exactly
        term.split_whitespace().any(|token| token == pattern)
            || pattern.split_whitespace().any(|token| token == term)
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
        if token.len() < 20
            && token.chars().all(|c| c.is_alphanumeric() || c == '_')
            && token.chars().any(|c| c.is_lowercase())
        {
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
        if (token.starts_with('"') && token.ends_with('"'))
            || (token.starts_with('\'') && token.ends_with('\''))
        {
            return "STRING_LIT".to_string();
        }

        token.to_string()
    }

    /// Get corpus statistics for analysis
    pub fn get_corpus_stats(&self) -> CorpusStatistics {
        CorpusStatistics {
            total_documents: self.total_documents,
            unique_terms: self.document_frequencies.len(),
            average_document_length: self.calculate_average_document_length(),
            vocabulary_diversity: self.calculate_vocabulary_diversity(),
        }
    }

    /// Calculate average document length
    fn calculate_average_document_length(&self) -> f64 {
        if self.total_documents == 0 {
            return 0.0;
        }

        let total_terms: usize = self
            .term_frequencies
            .values()
            .map(|tf_map| tf_map.len())
            .sum();

        total_terms as f64 / self.total_documents as f64
    }

    /// Calculate vocabulary diversity (unique terms / total terms)
    fn calculate_vocabulary_diversity(&self) -> f64 {
        let total_term_occurrences: usize = self.document_frequencies.values().sum();

        if total_term_occurrences == 0 {
            return 0.0;
        }

        self.document_frequencies.len() as f64 / total_term_occurrences as f64
    }
}

/// Statistics about the analyzed corpus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusStatistics {
    pub total_documents: usize,
    pub unique_terms: usize,
    pub average_document_length: f64,
    pub vocabulary_diversity: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfidf_analyzer_creation() {
        let analyzer = TfIdfAnalyzer::new();
        let stats = analyzer.get_corpus_stats();
        assert_eq!(stats.total_documents, 0);
        assert_eq!(stats.unique_terms, 0);
    }

    #[test]
    fn test_add_document() {
        let mut analyzer = TfIdfAnalyzer::new();
        analyzer.add_document(
            "doc1".to_string(),
            vec!["hello".to_string(), "world".to_string()],
        );

        let stats = analyzer.get_corpus_stats();
        assert_eq!(stats.total_documents, 1);
        assert!(stats.unique_terms > 0);
    }

    #[test]
    fn test_tfidf_calculation() {
        let config = NormalizationConfig {
            alpha_rename_locals: false, // Disable to keep "hello" and "world" distinct
            ..Default::default()
        };
        let mut analyzer = TfIdfAnalyzer::new_with_config(config);
        analyzer.add_document(
            "doc1".to_string(),
            vec!["hello".to_string(), "world".to_string()],
        );
        analyzer.add_document(
            "doc2".to_string(),
            vec!["hello".to_string(), "rust".to_string()],
        );

        let tfidf_hello = analyzer.tf_idf("doc1", "hello");
        let tfidf_world = analyzer.tf_idf("doc1", "world");

        // "hello" appears in both documents, so should have lower IDF
        // "world" appears in only one document, so should have higher IDF
        assert!(tfidf_world > tfidf_hello);
    }

    #[test]
    fn test_literal_bucketing() {
        let config = NormalizationConfig {
            bucket_literals: true,
            ..Default::default()
        };
        let analyzer = TfIdfAnalyzer::new_with_config(config);

        assert_eq!(analyzer.bucket_literal("123"), "INT_LIT");
        assert_eq!(analyzer.bucket_literal("123.456"), "FLOAT_LIT");
        assert_eq!(analyzer.bucket_literal("\"hello\""), "STRING_LIT");
        assert_eq!(analyzer.bucket_literal("variable_name"), "variable_name");
    }
}
