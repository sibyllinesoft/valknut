//! Language-agnostic normalization for clone detection

use serde::{Deserialize, Serialize};

/// Configuration for language-agnostic normalization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    /// Whether to perform alpha-renaming of local variables
    pub alpha_rename_locals: bool,

    /// Whether to bucket literal values (numbers, strings, etc.)
    pub bucket_literals: bool,

    /// Whether to normalize control flow structures
    pub normalize_control_flow: bool,

    /// Whether to remove language-specific keywords
    pub remove_keywords: bool,

    /// Whether to normalize function signatures
    pub normalize_signatures: bool,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            alpha_rename_locals: true,
            bucket_literals: true,
            normalize_control_flow: true,
            remove_keywords: false,
            normalize_signatures: true,
        }
    }
}

impl NormalizationConfig {
    /// Create a conservative normalization configuration
    pub fn conservative() -> Self {
        Self {
            alpha_rename_locals: false,
            bucket_literals: true,
            normalize_control_flow: false,
            remove_keywords: false,
            normalize_signatures: false,
        }
    }

    /// Create an aggressive normalization configuration
    pub fn aggressive() -> Self {
        Self {
            alpha_rename_locals: true,
            bucket_literals: true,
            normalize_control_flow: true,
            remove_keywords: true,
            normalize_signatures: true,
        }
    }

    /// Create a language-specific configuration
    pub fn for_language(language: &str) -> Self {
        match language.to_lowercase().as_str() {
            "python" => Self {
                alpha_rename_locals: true,
                bucket_literals: true,
                normalize_control_flow: true,
                remove_keywords: true,
                normalize_signatures: true,
            },
            "javascript" | "typescript" => Self {
                alpha_rename_locals: true,
                bucket_literals: true,
                normalize_control_flow: false, // JS has many control flow variations
                remove_keywords: false,
                normalize_signatures: true,
            },
            "rust" => Self {
                alpha_rename_locals: false, // Rust has strong typing
                bucket_literals: true,
                normalize_control_flow: true,
                remove_keywords: false,
                normalize_signatures: false, // Keep type information
            },
            "go" => Self {
                alpha_rename_locals: true,
                bucket_literals: true,
                normalize_control_flow: true,
                remove_keywords: false,
                normalize_signatures: true,
            },
            _ => Self::default(),
        }
    }
}

/// Language-agnostic code normalizer
#[derive(Debug, Clone)]
pub struct CodeNormalizer {
    config: NormalizationConfig,
}

impl CodeNormalizer {
    /// Create a new normalizer with the given configuration
    pub fn new(config: NormalizationConfig) -> Self {
        Self { config }
    }

    /// Normalize a piece of code according to the configuration
    pub fn normalize(&self, code: &str) -> String {
        let mut normalized = code.to_string();

        if self.config.bucket_literals {
            normalized = self.bucket_literals(&normalized);
        }

        if self.config.alpha_rename_locals {
            normalized = self.alpha_rename_locals(&normalized);
        }

        if self.config.normalize_control_flow {
            normalized = self.normalize_control_flow(&normalized);
        }

        if self.config.remove_keywords {
            normalized = self.remove_language_keywords(&normalized);
        }

        if self.config.normalize_signatures {
            normalized = self.normalize_function_signatures(&normalized);
        }

        normalized
    }

    /// Bucket literal values
    fn bucket_literals(&self, code: &str) -> String {
        // Simplified implementation without regex - using character-by-character parsing
        let mut result = String::with_capacity(code.len());
        let mut chars = code.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch.is_ascii_digit() {
                // Handle numeric literals
                let mut has_dot = false;
                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_ascii_digit() {
                        chars.next();
                    } else if next_ch == '.' && !has_dot {
                        has_dot = true;
                        chars.next();
                    } else {
                        break;
                    }
                }
                result.push_str(if has_dot { "FLOAT_LIT" } else { "INT_LIT" });
            } else if ch == '"' {
                // Handle double-quoted strings
                while let Some(string_ch) = chars.next() {
                    if string_ch == '"' {
                        break;
                    }
                }
                result.push_str("STRING_LIT");
            } else if ch == '\'' {
                // Handle single-quoted strings
                while let Some(string_ch) = chars.next() {
                    if string_ch == '\'' {
                        break;
                    }
                }
                result.push_str("STRING_LIT");
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Alpha-rename local variables
    fn alpha_rename_locals(&self, code: &str) -> String {
        // Simplified implementation using word boundary detection
        let mut result = String::with_capacity(code.len());
        let mut current_word = String::new();

        for ch in code.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                current_word.push(ch);
            } else {
                if !current_word.is_empty() {
                    if current_word
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_ascii_lowercase() || c == '_')
                        && self.is_likely_local_variable(&current_word)
                    {
                        result.push_str("VAR");
                    } else {
                        result.push_str(&current_word);
                    }
                    current_word.clear();
                }
                result.push(ch);
            }
        }

        // Handle final word
        if !current_word.is_empty() {
            if current_word
                .chars()
                .next()
                .map_or(false, |c| c.is_ascii_lowercase() || c == '_')
                && self.is_likely_local_variable(&current_word)
            {
                result.push_str("VAR");
            } else {
                result.push_str(&current_word);
            }
        }

        result
    }

    /// Check if a word is likely a local variable
    fn is_likely_local_variable(&self, word: &str) -> bool {
        // Skip common keywords and built-in functions
        const COMMON_WORDS: &[&str] = &[
            "if",
            "else",
            "for",
            "while",
            "do",
            "switch",
            "case",
            "default",
            "function",
            "return",
            "var",
            "let",
            "const",
            "class",
            "struct",
            "impl",
            "trait",
            "enum",
            "match",
            "true",
            "false",
            "null",
            "undefined",
            "print",
            "println",
            "console",
            "log",
            "length",
            "size",
            "push",
            "pop",
            "get",
            "set",
            "add",
            "remove",
            "contains",
            "empty",
        ];

        if COMMON_WORDS.contains(&word) {
            return false;
        }

        // Likely local variable if it's short and contains lowercase
        word.len() < 20 && word.chars().any(|c| c.is_lowercase())
    }

    /// Normalize control flow structures
    fn normalize_control_flow(&self, code: &str) -> String {
        // Simple pattern replacement without regex
        let mut result = code.to_string();

        // Normalize for loops - simple keyword-based replacement
        result = self.replace_control_structure(&result, "for", "for(INIT; COND; UPDATE)");

        // Normalize while loops
        result = self.replace_control_structure(&result, "while", "while(COND)");

        // Normalize if statements
        result = self.replace_control_structure(&result, "if", "if(COND)");

        result
    }

    /// Helper function to replace control structures
    fn replace_control_structure(&self, code: &str, keyword: &str, replacement: &str) -> String {
        let mut result = String::new();
        let mut chars = code.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch.is_ascii_alphabetic() {
                let mut word = String::new();
                word.push(ch);

                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_alphanumeric() || next_ch == '_' {
                        word.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                if word == keyword {
                    // Skip whitespace and find opening parenthesis
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch == '(' {
                            chars.next(); // consume '('
                            let mut paren_count = 1;
                            // Skip until matching closing parenthesis
                            while paren_count > 0 && chars.peek().is_some() {
                                match chars.next().unwrap() {
                                    '(' => paren_count += 1,
                                    ')' => paren_count -= 1,
                                    _ => {}
                                }
                            }
                            result.push_str(replacement);
                            break;
                        } else if next_ch.is_whitespace() {
                            chars.next(); // consume whitespace
                        } else {
                            result.push_str(&word);
                            break;
                        }
                    }
                } else {
                    result.push_str(&word);
                }
            } else {
                result.push(ch);
            }
        }

        result
    }

    /// Remove language-specific keywords
    fn remove_language_keywords(&self, code: &str) -> String {
        // Remove common language-specific keywords that don't affect structure
        const KEYWORDS_TO_REMOVE: &[&str] = &[
            "public",
            "private",
            "protected",
            "static",
            "final",
            "abstract",
            "virtual",
            "override",
            "async",
            "await",
            "const",
            "mutable",
            "inline",
            "extern",
            "unsafe",
            "mut",
            "ref",
            "out",
        ];

        let mut result = String::new();
        let mut current_word = String::new();

        for ch in code.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                current_word.push(ch);
            } else {
                if !current_word.is_empty() {
                    if !KEYWORDS_TO_REMOVE.contains(&current_word.as_str()) {
                        result.push_str(&current_word);
                    }
                    current_word.clear();
                }
                result.push(ch);
            }
        }

        // Handle final word
        if !current_word.is_empty() && !KEYWORDS_TO_REMOVE.contains(&current_word.as_str()) {
            result.push_str(&current_word);
        }

        // Clean up extra whitespace by replacing multiple spaces with single space
        let mut cleaned = String::new();
        let mut prev_space = false;
        for ch in result.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    cleaned.push(' ');
                    prev_space = true;
                }
            } else {
                cleaned.push(ch);
                prev_space = false;
            }
        }

        cleaned
    }

    /// Normalize function signatures
    fn normalize_function_signatures(&self, code: &str) -> String {
        let mut result = String::new();
        let mut chars = code.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '(' {
                // Skip everything until matching closing parenthesis
                let mut paren_count = 1;
                while paren_count > 0 && chars.peek().is_some() {
                    match chars.next().unwrap() {
                        '(' => paren_count += 1,
                        ')' => paren_count -= 1,
                        _ => {}
                    }
                }
                result.push_str("(PARAMS)");
            } else if ch == '<' {
                // Skip everything until matching closing angle bracket
                let mut angle_count = 1;
                while angle_count > 0 && chars.peek().is_some() {
                    match chars.next().unwrap() {
                        '<' => angle_count += 1,
                        '>' => angle_count -= 1,
                        _ => {}
                    }
                }
                result.push_str("<TYPES>");
            } else {
                result.push(ch);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalization_config_default() {
        let config = NormalizationConfig::default();
        assert!(config.alpha_rename_locals);
        assert!(config.bucket_literals);
    }

    #[test]
    fn test_language_specific_config() {
        let python_config = NormalizationConfig::for_language("python");
        let rust_config = NormalizationConfig::for_language("rust");

        assert!(python_config.alpha_rename_locals);
        assert!(!rust_config.alpha_rename_locals); // Rust has strong typing
    }

    #[test]
    fn test_bucket_literals() {
        let normalizer = CodeNormalizer::new(NormalizationConfig::default());
        let input = "x = 42; y = 3.14; z = \"hello\";";
        let output = normalizer.bucket_literals(input);

        assert!(output.contains("INT_LIT"));
        assert!(output.contains("FLOAT_LIT"));
        assert!(output.contains("STRING_LIT"));
    }

    #[test]
    fn test_is_likely_local_variable() {
        let normalizer = CodeNormalizer::new(NormalizationConfig::default());

        assert!(normalizer.is_likely_local_variable("temp"));
        assert!(normalizer.is_likely_local_variable("count"));
        assert!(!normalizer.is_likely_local_variable("if"));
        assert!(!normalizer.is_likely_local_variable("function"));
    }

    #[test]
    fn test_normalize_control_flow() {
        let normalizer = CodeNormalizer::new(NormalizationConfig::default());
        let input = "for(int i = 0; i < 10; i++) { while(condition) { } }";
        let output = normalizer.normalize_control_flow(input);

        assert!(output.contains("for(INIT; COND; UPDATE)"));
        assert!(output.contains("while(COND)"));
    }

    #[test]
    fn test_full_normalization() {
        let normalizer = CodeNormalizer::new(NormalizationConfig::aggressive());
        let input = "public static void method(int param) { x = 42; }";
        let output = normalizer.normalize(input);

        // Should remove access modifiers, bucket literals, and normalize variables
        assert!(!output.contains("public"));
        assert!(!output.contains("static"));
        assert!(output.contains("INT_LIT"));
    }
}
