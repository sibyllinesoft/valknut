//! Shingle creation and code normalization for LSH analysis.
//!
//! This module provides functionality for creating shingles (n-grams of tokens)
//! from source code, which are used for MinHash signature generation.

use tracing::debug;

use crate::core::interning::{intern, resolve, InternedString};

use super::super::lsh_cache::LshCache;
use super::super::memory_pool::LshMemoryPools;

/// Shingle generator for creating n-grams from source code.
pub struct ShingleGenerator {
    /// Shingle size (number of tokens per shingle)
    shingle_size: usize,
}

/// Factory and shingle generation methods for [`ShingleGenerator`].
impl ShingleGenerator {
    /// Create a new shingle generator with the given shingle size.
    pub fn new(shingle_size: usize) -> Self {
        Self { shingle_size }
    }

    /// Create shingles from source code.
    pub fn create_shingles(&self, source_code: &str) -> Vec<String> {
        // Normalize the source code (remove comments, normalize whitespace)
        let normalized = self.normalize_code(source_code);

        // Split into tokens
        let tokens: Vec<&str> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .collect();

        // Create shingles
        let mut shingles = Vec::new();
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                let shingle = tokens[i..i + self.shingle_size].join(" ");
                shingles.push(shingle);
            }
        }

        shingles
    }

    /// Create interned shingles from source code for zero-allocation performance.
    /// This is the high-performance version that uses string interning.
    pub fn create_shingles_interned(&self, source_code: &str) -> Vec<InternedString> {
        // Normalize the source code (remove comments, normalize whitespace)
        let normalized = self.normalize_code(source_code);

        // Split into tokens and intern them immediately
        let tokens: Vec<InternedString> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(|token| intern(token)) // ZERO allocations - intern directly from &str
            .collect();

        // Create shingles by combining interned tokens
        let mut shingles = Vec::new();
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                // Build shingle by resolving tokens and joining - only one allocation per shingle
                let shingle_parts: Vec<&str> = tokens[i..i + self.shingle_size]
                    .iter()
                    .map(|&interned_token| resolve(interned_token))
                    .collect();
                let shingle_str = shingle_parts.join(" ");
                let interned_shingle = intern(shingle_str);
                shingles.push(interned_shingle);
            }
        }

        shingles
    }

    /// Create shingles using memory pools to reduce allocation churn.
    pub fn create_shingles_pooled(
        &self,
        source_code: &str,
        memory_pools: &LshMemoryPools,
    ) -> Vec<String> {
        // Normalize the source code
        let normalized = self.normalize_code(source_code);

        // Split into tokens
        let tokens: Vec<&str> = normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .collect();

        // Create shingles using memory pool
        let mut shingles = memory_pools.get_string_vec();
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                let shingle = tokens[i..i + self.shingle_size].join(" ");
                shingles.push(shingle);
            }
        }

        shingles
    }

    /// Create shingles with token caching to avoid redundant tokenization.
    pub fn create_shingles_cached(
        &self,
        source_code: &str,
        cache: &LshCache,
        memory_pools: &LshMemoryPools,
    ) -> Vec<String> {
        // Check token cache first
        if let Some(cached_tokens) = cache.get_tokens(source_code) {
            debug!("Token cache hit for source code");
            return self.tokens_to_shingles(cached_tokens, memory_pools);
        }

        // Generate tokens and shingles using memory pool
        let normalized = self.normalize_code(source_code);
        let mut tokens = memory_pools.get_string_vec();
        tokens.extend(
            normalized
                .split_whitespace()
                .filter(|token| !token.is_empty())
                .map(|s| s.to_string()),
        );

        // Cache the tokens for future use
        cache.cache_tokens(source_code, tokens.clone());

        // Convert tokens to shingles (returns tokens to pool internally)
        self.tokens_to_shingles(tokens, memory_pools)
    }

    /// Convert tokens to shingles.
    pub fn tokens_to_shingles(
        &self,
        tokens: Vec<String>,
        memory_pools: &LshMemoryPools,
    ) -> Vec<String> {
        let mut shingles = memory_pools.get_string_vec();
        if tokens.len() >= self.shingle_size {
            for i in 0..=tokens.len() - self.shingle_size {
                let shingle = tokens[i..i + self.shingle_size].join(" ");
                shingles.push(shingle);
            }
        }

        // Return tokens vector to pool for reuse
        memory_pools.return_string_vec(tokens);

        shingles
    }

    /// Normalize source code for comparison using basic text processing.
    pub fn normalize_code(&self, source_code: &str) -> String {
        super::generator::normalize_code(source_code)
    }
}

/// Count tokens in source code (simplified approach).
pub fn count_tokens(source_code: &str) -> usize {
    source_code
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_shingles() {
        let generator = ShingleGenerator::new(3);
        let code = "fn main() { println!(\"Hello\"); }";
        let shingles = generator.create_shingles(code);
        assert!(!shingles.is_empty());
    }

    #[test]
    fn test_normalize_code() {
        let generator = ShingleGenerator::new(3);
        let code = "// Comment\nfn main() {\n    let x = 1;\n}";
        let normalized = generator.normalize_code(code);
        assert!(!normalized.contains("//"));
        assert!(normalized.contains("fn"));
    }

    #[test]
    fn test_count_tokens() {
        let code = "fn main() { let x = 1; }";
        let count = count_tokens(code);
        assert_eq!(count, 8);
    }
}
