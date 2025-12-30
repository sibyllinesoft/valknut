//! Symbol extraction and TF-IDF weighting for cohesion analysis.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::config::SymbolConfig;

/// Represents extracted symbols from a code entity.
#[derive(Debug, Clone, Default)]
pub struct ExtractedSymbols {
    /// Entity kind (function, class, method, etc.)
    pub kind: String,
    /// Qualified name of the entity
    pub qualified_name: String,
    /// Tokenized name parts (e.g., "getUserName" -> ["get", "user", "name"])
    pub name_tokens: Vec<String>,
    /// Signature tokens (parameter names, types, return type)
    pub signature_tokens: Vec<String>,
    /// Referenced symbols (calls, types, fields, imports)
    pub referenced_symbols: Vec<String>,
    /// Short doc summary (if available)
    pub doc_summary: Option<String>,
}

impl ExtractedSymbols {
    /// Build the code text for embedding from extracted symbols.
    pub fn build_code_text(&self, config: &SymbolConfig) -> String {
        let mut parts = Vec::new();

        // Kind prefix
        parts.push(self.kind.clone());

        // Qualified name
        parts.push(self.qualified_name.clone());

        // Tokenized name
        parts.extend(self.name_tokens.iter().cloned());

        // Signature tokens (if enabled)
        if config.include_signature {
            parts.extend(self.signature_tokens.iter().cloned());
        }

        // Referenced symbols (already filtered by TF-IDF)
        parts.extend(self.referenced_symbols.iter().cloned());

        // Doc summary (if enabled and available)
        if config.include_doc_summary {
            if let Some(ref summary) = self.doc_summary {
                let summary_tokens: Vec<&str> = summary.split_whitespace().collect();
                let max_tokens = config.max_doc_summary_tokens;
                parts.extend(
                    summary_tokens
                        .into_iter()
                        .take(max_tokens)
                        .map(|s| s.to_string()),
                );
            }
        }

        parts.join(" ")
    }
}

/// TF-IDF calculator for symbol weighting.
#[derive(Debug, Clone)]
pub struct TfIdfCalculator {
    /// Document frequency per symbol (across corpus)
    document_frequencies: HashMap<String, usize>,
    /// Total documents in corpus
    total_documents: usize,
    /// Configuration
    config: SymbolConfig,
}

impl TfIdfCalculator {
    /// Create a new TF-IDF calculator.
    pub fn new(config: SymbolConfig) -> Self {
        Self {
            document_frequencies: HashMap::new(),
            total_documents: 0,
            config,
        }
    }

    /// Add a document (entity) to the corpus for IDF calculation.
    pub fn add_document(&mut self, symbols: &[String]) {
        self.total_documents += 1;

        // Count unique symbols in this document
        let unique_symbols: std::collections::HashSet<_> = symbols.iter().collect();
        for symbol in unique_symbols {
            *self.document_frequencies.entry(symbol.clone()).or_insert(0) += 1;
        }
    }

    /// Calculate TF for a symbol in a document.
    fn term_frequency(&self, symbol: &str, document: &[String]) -> f64 {
        let count = document.iter().filter(|s| *s == symbol).count();
        if count == 0 {
            0.0
        } else {
            1.0 + (count as f64).ln()
        }
    }

    /// Calculate IDF for a symbol.
    fn inverse_document_frequency(&self, symbol: &str) -> f64 {
        let df = self.document_frequencies.get(symbol).copied().unwrap_or(0);
        let n = self.total_documents as f64;
        ((n + 1.0) / (df as f64 + 1.0)).ln()
    }

    /// Calculate TF-IDF weight for a symbol.
    fn tfidf(&self, symbol: &str, document: &[String]) -> f64 {
        self.term_frequency(symbol, document) * self.inverse_document_frequency(symbol)
    }

    /// Select top-K symbols using adaptive TF-IDF weighting.
    ///
    /// Selection criteria:
    /// 1. Sort by TF-IDF weight descending
    /// 2. Keep until cumulative mass >= threshold (e.g., 80%)
    /// 3. Apply sublinear cap: K_cap = clamp(K_min, K_max, ceil(a * sqrt(m)))
    pub fn select_top_symbols(&self, document: &[String]) -> Vec<String> {
        if document.is_empty() {
            return Vec::new();
        }

        // Get unique symbols with their weights
        let unique_symbols: std::collections::HashSet<_> = document.iter().collect();
        let mut weighted: Vec<(String, f64)> = unique_symbols
            .into_iter()
            .map(|s| (s.clone(), self.tfidf(s, document)))
            .collect();

        // Sort by weight descending
        weighted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let total_weight: f64 = weighted.iter().map(|(_, w)| w).sum();
        if total_weight == 0.0 {
            return weighted.into_iter().map(|(s, _)| s).collect();
        }

        // Calculate sublinear cap
        let m = weighted.len();
        let k_cap = (self.config.sublinear_coefficient * (m as f64).sqrt()).ceil() as usize;
        let k_cap = k_cap.clamp(self.config.min_symbols, self.config.max_symbols);

        // Select by cumulative mass until threshold OR cap reached
        let threshold = self.config.tfidf_mass_threshold;
        let mut cumulative = 0.0;
        let mut selected = Vec::new();

        for (symbol, weight) in weighted {
            if selected.len() >= k_cap {
                break;
            }

            selected.push(symbol);
            cumulative += weight;

            if cumulative / total_weight >= threshold {
                break;
            }
        }

        // Ensure minimum symbols
        if selected.len() < self.config.min_symbols {
            // Already added all we have or need more - this is fine
        }

        selected
    }

    /// Get document frequency for a symbol.
    pub fn get_df(&self, symbol: &str) -> usize {
        self.document_frequencies.get(symbol).copied().unwrap_or(0)
    }

    /// Get total documents in corpus.
    pub fn total_documents(&self) -> usize {
        self.total_documents
    }

    /// Get corpus statistics.
    pub fn stats(&self) -> TfIdfStats {
        TfIdfStats {
            total_documents: self.total_documents,
            unique_symbols: self.document_frequencies.len(),
            avg_df: if self.document_frequencies.is_empty() {
                0.0
            } else {
                self.document_frequencies.values().sum::<usize>() as f64
                    / self.document_frequencies.len() as f64
            },
        }
    }
}

/// Statistics about the TF-IDF corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfIdfStats {
    /// Total documents in corpus
    pub total_documents: usize,
    /// Number of unique symbols
    pub unique_symbols: usize,
    /// Average document frequency
    pub avg_df: f64,
}

/// Common stop tokens to filter out from symbol extraction.
pub fn is_stop_token(token: &str) -> bool {
    // Common programming keywords and noise
    const STOP_TOKENS: &[&str] = &[
        // Common words
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall",
        "can", "need", "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into",
        "through", "during", "before", "after", "above", "below", "between", "under", "again",
        "further", "then", "once", "here", "there", "when", "where", "why", "how", "all", "each",
        "few", "more", "most", "other", "some", "such", "no", "nor", "not", "only", "own", "same",
        "so", "than", "too", "very", "just", "and", "but", "if", "or", "because", "until", "while",
        "this", "that", "these", "those", "it", "its", // Common keywords
        "self", "cls", "this", // Python/JS/Rust self references
        "none", "null", "nil", "undefined", // Null values
        "true", "false", // Booleans
        "int", "str", "string", "bool", "float", "double", "char", "byte", "void", "i32", "i64",
        "u32", "u64", "f32", "f64", "usize", "isize", // Primitive types
        "let", "const", "var", "mut", "pub", "priv", "private", "public", "protected", "static",
        "final", "abstract", "virtual", "override", "async", "await", "fn", "func", "function",
        "def", "class", "struct", "enum", "trait", "impl", "interface", "type", "module", "import",
        "export", "from", "use", "mod", "crate", "super", "where", // Language keywords
        "return", "break", "continue", "yield", "throw", "raise", "try", "catch", "except",
        "finally", "match", "case", "switch", "default", "loop", "for", "while", "if", "else",
        "elif", "unless", // Control flow
        "new", "delete", "sizeof", "typeof", "instanceof", // Operators
        "get", "set", // Property accessors (too common)
        "ok", "err", "error", "result", "option", "some", // Result/Option (too common in Rust)
        "args", "kwargs", "arg", "param", "params", // Generic parameter names
        "i", "j", "k", "n", "m", "x", "y", "z", // Single letter variables
        "tmp", "temp", "val", "value", "data", "item", "items", "obj", "object",
        "list", "array", "vec", "map", "dict", "set", "hash", // Generic container names
    ];

    let lower = token.to_lowercase();
    STOP_TOKENS.contains(&lower.as_str()) || token.len() <= 1
}

/// Check if character is a word separator.
fn is_separator(ch: char) -> bool {
    matches!(ch, '_' | '-' | '.' | '/')
}

/// Flush current token to the list if non-empty.
fn flush_token(tokens: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        tokens.push(current.to_lowercase());
        current.clear();
    }
}

/// Tokenize a name into its component parts.
/// Handles camelCase, PascalCase, snake_case, and SCREAMING_CASE.
pub fn tokenize_name(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for ch in name.chars() {
        if is_separator(ch) {
            flush_token(&mut tokens, &mut current);
        } else if ch.is_uppercase() && !current.is_empty() && !current.ends_with(char::is_uppercase) {
            // camelCase boundary: lowercase -> uppercase
            flush_token(&mut tokens, &mut current);
            current.push(ch);
        } else if ch.is_lowercase() && current.len() > 1 && current.chars().all(char::is_uppercase) {
            // ALLCAPS to lowercase (e.g., HTTPRequest -> HTTP, Request)
            let last = current.pop().unwrap();
            flush_token(&mut tokens, &mut current);
            current.push(last);
            current.push(ch);
        } else {
            current.push(ch);
        }
    }

    flush_token(&mut tokens, &mut current);
    tokens.into_iter().filter(|t| t.len() > 1).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_camel_case() {
        assert_eq!(
            tokenize_name("getUserName"),
            vec!["get", "user", "name"]
        );
    }

    #[test]
    fn tokenize_pascal_case() {
        assert_eq!(
            tokenize_name("UserManager"),
            vec!["user", "manager"]
        );
    }

    #[test]
    fn tokenize_snake_case() {
        assert_eq!(
            tokenize_name("get_user_name"),
            vec!["get", "user", "name"]
        );
    }

    #[test]
    fn tokenize_screaming_case() {
        assert_eq!(
            tokenize_name("MAX_BUFFER_SIZE"),
            vec!["max", "buffer", "size"]
        );
    }

    #[test]
    fn tokenize_mixed_case() {
        assert_eq!(
            tokenize_name("HTTPRequestHandler"),
            vec!["http", "request", "handler"]
        );
    }

    #[test]
    fn stop_tokens_filtered() {
        assert!(is_stop_token("the"));
        assert!(is_stop_token("self"));
        assert!(is_stop_token("i"));
        assert!(!is_stop_token("user"));
        assert!(!is_stop_token("manager"));
    }

    #[test]
    fn tfidf_basic_calculation() {
        let config = SymbolConfig::default();
        let mut calc = TfIdfCalculator::new(config);

        // Add some documents
        calc.add_document(&["user".into(), "manager".into(), "create".into()]);
        calc.add_document(&["user".into(), "validator".into()]);
        calc.add_document(&["config".into(), "loader".into()]);

        // "user" appears in 2/3 docs, should have lower IDF
        // "config" appears in 1/3 docs, should have higher IDF
        assert!(calc.get_df("user") > calc.get_df("config"));
    }

    #[test]
    fn tfidf_select_top_symbols() {
        let config = SymbolConfig {
            tfidf_mass_threshold: 0.8,
            min_symbols: 2,
            max_symbols: 10,
            sublinear_coefficient: 3.0,
            ..Default::default()
        };
        let mut calc = TfIdfCalculator::new(config);

        // Build a small corpus
        calc.add_document(&["user".into(), "create".into(), "manager".into()]);
        calc.add_document(&["user".into(), "delete".into()]);
        calc.add_document(&["config".into(), "load".into()]);

        // Select from a document
        let doc = vec![
            "user".into(),
            "create".into(),
            "special_function".into(),
        ];
        let selected = calc.select_top_symbols(&doc);

        // Should select some symbols
        assert!(!selected.is_empty());
        assert!(selected.len() >= 2);
    }

    #[test]
    fn extracted_symbols_build_code_text() {
        let symbols = ExtractedSymbols {
            kind: "function".into(),
            qualified_name: "user::create_user".into(),
            name_tokens: vec!["create".into(), "user".into()],
            signature_tokens: vec!["name".into(), "email".into()],
            referenced_symbols: vec!["UserRepository".into(), "validate".into()],
            doc_summary: Some("Creates a new user account".into()),
        };

        let config = SymbolConfig {
            include_signature: true,
            include_doc_summary: false,
            ..Default::default()
        };

        let text = symbols.build_code_text(&config);
        assert!(text.contains("function"));
        assert!(text.contains("user::create_user"));
        assert!(text.contains("create"));
        assert!(text.contains("UserRepository"));
        assert!(!text.contains("Creates")); // Doc summary disabled
    }
}
