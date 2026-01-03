//! Embedding generation using fastembed with caching.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use fastembed::{EmbeddingModel as FastEmbedModel, InitOptions, TextEmbedding};

use crate::core::errors::{Result, ValknutError};

use super::config::{EmbeddingConfig, EmbeddingModel};

/// Provider for generating text embeddings using fastembed.
pub struct EmbeddingProvider {
    model: RwLock<TextEmbedding>,
    dimension: usize,
    cache: Arc<RwLock<EmbeddingCache>>,
}

/// In-memory cache for embeddings (hash -> vector)
struct EmbeddingCache {
    entries: HashMap<u64, Vec<f32>>,
    max_entries: usize,
    hits: usize,
    misses: usize,
}

/// Factory, lookup, and eviction methods for [`EmbeddingCache`].
impl EmbeddingCache {
    /// Creates a new cache with the given maximum entry count.
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    /// Retrieves an embedding from the cache if present.
    fn get(&mut self, hash: u64) -> Option<Vec<f32>> {
        if let Some(vec) = self.entries.get(&hash) {
            self.hits += 1;
            Some(vec.clone())
        } else {
            self.misses += 1;
            None
        }
    }

    /// Inserts an embedding into the cache, evicting entries if at capacity.
    fn insert(&mut self, hash: u64, embedding: Vec<f32>) {
        // Simple eviction: if at capacity, clear half the cache
        if self.entries.len() >= self.max_entries {
            let to_remove: Vec<u64> = self
                .entries
                .keys()
                .take(self.max_entries / 2)
                .copied()
                .collect();
            for key in to_remove {
                self.entries.remove(&key);
            }
        }
        self.entries.insert(hash, embedding);
    }

    /// Computes the cache hit rate.
    fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Factory, embedding generation, and caching methods for [`EmbeddingProvider`].
impl EmbeddingProvider {
    /// Create a new embedding provider with the given configuration.
    pub fn new(config: &EmbeddingConfig) -> Result<Self> {
        let fastembed_model = config.model.to_fastembed_model();
        let dimension = config.model.dimension();

        let mut init_options = InitOptions::new(fastembed_model);
        init_options = init_options.with_show_download_progress(config.show_download_progress);

        if let Some(ref cache_dir) = config.cache_dir {
            init_options = init_options.with_cache_dir(cache_dir.into());
        }

        let model = TextEmbedding::try_new(init_options).map_err(|e| {
            ValknutError::internal(format!("Failed to initialize embedding model: {}", e))
        })?;

        // Cache size: ~10k embeddings at 768 dim = ~30MB
        let max_cache_entries = 10_000;

        Ok(Self {
            model: RwLock::new(model),
            dimension,
            cache: Arc::new(RwLock::new(EmbeddingCache::new(max_cache_entries))),
        })
    }

    /// Get the embedding dimension.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Generate embedding for a single text.
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        let hash = Self::hash_text(text);

        // Check cache first
        {
            let mut cache = self.cache.write().map_err(|e| {
                ValknutError::internal(format!("Failed to acquire cache lock: {}", e))
            })?;
            if let Some(embedding) = cache.get(hash) {
                return Ok(embedding);
            }
        }

        // Generate embedding
        let embeddings = {
            let mut model = self.model.write().map_err(|e| {
                ValknutError::internal(format!("Failed to acquire model lock: {}", e))
            })?;
            model
                .embed(vec![text], None)
                .map_err(|e| ValknutError::internal(format!("Embedding generation failed: {}", e)))?
        };

        let embedding = embeddings.into_iter().next().ok_or_else(|| {
            ValknutError::internal("Embedding generation returned empty result".to_string())
        })?;

        // Cache the result
        {
            let mut cache = self.cache.write().map_err(|e| {
                ValknutError::internal(format!("Failed to acquire cache lock: {}", e))
            })?;
            cache.insert(hash, embedding.clone());
        }

        Ok(embedding)
    }

    /// Generate embeddings for multiple texts in a batch.
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Separate cached and uncached
        let mut results: Vec<Option<Vec<f32>>> = vec![None; texts.len()];
        let mut uncached_indices: Vec<usize> = Vec::new();
        let mut uncached_texts: Vec<String> = Vec::new();

        {
            let mut cache = self.cache.write().map_err(|e| {
                ValknutError::internal(format!("Failed to acquire cache lock: {}", e))
            })?;

            for (i, text) in texts.iter().enumerate() {
                let hash = Self::hash_text(text);
                if let Some(embedding) = cache.get(hash) {
                    results[i] = Some(embedding);
                } else {
                    uncached_indices.push(i);
                    uncached_texts.push(text.clone());
                }
            }
        }

        // Generate embeddings for uncached texts
        if !uncached_texts.is_empty() {
            let text_refs: Vec<&str> = uncached_texts.iter().map(|s| s.as_str()).collect();
            let new_embeddings = {
                let mut model = self.model.write().map_err(|e| {
                    ValknutError::internal(format!("Failed to acquire model lock: {}", e))
                })?;
                model
                    .embed(text_refs, None)
                    .map_err(|e| ValknutError::internal(format!("Batch embedding failed: {}", e)))?
            };

            // Cache and store results
            {
                let mut cache = self.cache.write().map_err(|e| {
                    ValknutError::internal(format!("Failed to acquire cache lock: {}", e))
                })?;

                for (i, embedding) in uncached_indices.into_iter().zip(new_embeddings.into_iter())
                {
                    let hash = Self::hash_text(&texts[i]);
                    cache.insert(hash, embedding.clone());
                    results[i] = Some(embedding);
                }
            }
        }

        // Unwrap all results
        results
            .into_iter()
            .enumerate()
            .map(|(i, opt)| {
                opt.ok_or_else(|| {
                    ValknutError::internal(format!("Missing embedding for text at index {}", i))
                })
            })
            .collect()
    }

    /// Get cache statistics.
    pub fn cache_stats(&self) -> (usize, f64) {
        let cache = self.cache.read().unwrap();
        (cache.entries.len(), cache.hit_rate())
    }

    /// Hash text for cache key using xxhash.
    fn hash_text(text: &str) -> u64 {
        use xxhash_rust::xxh3::xxh3_64;
        xxh3_64(text.as_bytes())
    }
}

/// Conversion methods for [`EmbeddingModel`].
impl EmbeddingModel {
    /// Convert to fastembed model enum.
    pub fn to_fastembed_model(&self) -> FastEmbedModel {
        match self {
            EmbeddingModel::EmbeddingGemma300M => FastEmbedModel::EmbeddingGemma300M,
            EmbeddingModel::BGESmallENV15 => FastEmbedModel::BGESmallENV15,
            EmbeddingModel::BGESmallENV15Q => FastEmbedModel::BGESmallENV15Q,
            EmbeddingModel::AllMiniLML6V2 => FastEmbedModel::AllMiniLML6V2,
            EmbeddingModel::AllMiniLML6V2Q => FastEmbedModel::AllMiniLML6V2Q,
            EmbeddingModel::NomicEmbedTextV15 => FastEmbedModel::NomicEmbedTextV15,
            EmbeddingModel::JinaEmbeddingsV2BaseCode => FastEmbedModel::JinaEmbeddingsV2BaseCode,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedding_cache_basic_operations() {
        let mut cache = EmbeddingCache::new(100);

        // Miss on first access
        assert!(cache.get(123).is_none());
        assert_eq!(cache.misses, 1);

        // Insert and hit
        cache.insert(123, vec![1.0, 2.0, 3.0]);
        let result = cache.get(123);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), vec![1.0, 2.0, 3.0]);
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn embedding_cache_eviction() {
        let mut cache = EmbeddingCache::new(10);

        // Fill cache
        for i in 0..10 {
            cache.insert(i, vec![i as f32]);
        }
        assert_eq!(cache.entries.len(), 10);

        // Insert one more triggers eviction
        cache.insert(100, vec![100.0]);
        assert!(cache.entries.len() <= 6); // Half evicted + 1 new
    }

    #[test]
    fn hash_text_is_deterministic() {
        let hash1 = EmbeddingProvider::hash_text("hello world");
        let hash2 = EmbeddingProvider::hash_text("hello world");
        let hash3 = EmbeddingProvider::hash_text("hello world!");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
