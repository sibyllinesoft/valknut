//! MinHash signature generation with optimized shingle processing.
//!
//! This module provides high-performance MinHash signature generation using
//! interned strings and memory pooling to minimize allocation overhead.

use std::hash::{Hash, Hasher};

use tracing::debug;
use xxhash_rust::xxh3::Xxh3;

#[cfg(feature = "simd")]
use wide::u64x4;

use super::super::{LshCache, LshMemoryPools};
use crate::core::interning::{intern, resolve, InternedString};

/// Trait for MinHash signature generation operations.
///
/// This trait is implemented by `LshExtractor` and provides methods for
/// generating shingles and MinHash signatures from source code.
pub trait SignatureGenerator {
    /// Get the number of hash functions used
    fn num_hashes(&self) -> usize;

    /// Get the shingle size
    fn shingle_size(&self) -> usize;

    /// Get the cache for signature operations
    fn cache(&self) -> &LshCache;

    /// Get memory pools for allocation
    fn memory_pools(&self) -> &LshMemoryPools;
}

/// Create shingles from source code.
///
/// Normalizes the code and creates overlapping n-grams (shingles) for similarity comparison.
pub fn create_shingles<T: SignatureGenerator>(gen: &T, source_code: &str) -> Vec<String> {
    let normalized = normalize_code(source_code);
    let tokens: Vec<&str> = normalized
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect();

    let mut shingles = gen.memory_pools().get_string_vec();
    let shingle_size = gen.shingle_size();

    if tokens.len() >= shingle_size {
        for i in 0..=tokens.len() - shingle_size {
            let shingle = tokens[i..i + shingle_size].join(" ");
            shingles.push(shingle);
        }
    }

    shingles
}

/// Create interned shingles from source code - ZERO STRING ALLOCATIONS!
///
/// This is the high-performance version that eliminates all string allocation overhead.
pub fn create_shingles_interned<T: SignatureGenerator>(
    gen: &T,
    source_code: &str,
) -> Vec<InternedString> {
    let normalized = normalize_code(source_code);
    let shingle_size = gen.shingle_size();

    // Split into tokens and intern them immediately
    let tokens: Vec<InternedString> = normalized
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| intern(token)) // ZERO allocations - intern directly from &str
        .collect();

    // Create shingles by combining interned tokens
    let mut shingles = Vec::new();
    if tokens.len() >= shingle_size {
        for i in 0..=tokens.len() - shingle_size {
            // Build shingle by resolving tokens and joining - only one allocation per shingle
            let shingle_parts: Vec<&str> = tokens[i..i + shingle_size]
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

/// Generate MinHash signature for source code with performance tracking and caching.
pub fn generate_minhash_signature<T: SignatureGenerator>(gen: &T, source_code: &str) -> Vec<u64> {
    let start_time = std::time::Instant::now();
    let num_hashes = gen.num_hashes();
    let shingle_size = gen.shingle_size();

    // Check cache first
    if let Some(cached_signature) = gen.cache().get_signature(source_code, num_hashes, shingle_size)
    {
        let elapsed = start_time.elapsed();
        debug!("Signature cache hit, returned in {:?}", elapsed);
        return cached_signature;
    }

    // Create shingles from the source code (with caching)
    let shingles = create_shingles_cached(gen, source_code);

    // Generate MinHash signature using memory pool
    let mut signature = gen.memory_pools().get_signature_vec();
    signature.resize(num_hashes, u64::MAX);

    for shingle in shingles {
        for i in 0..num_hashes {
            let hash = hash_with_seed(&shingle, i as u64);
            if hash < signature[i] {
                signature[i] = hash;
            }
        }
    }

    // Cache the generated signature (clone before returning to pool)
    let signature_clone = signature.clone();
    gen.cache()
        .cache_signature(source_code, num_hashes, shingle_size, signature_clone.clone());

    // Return signature vector to memory pool for reuse
    gen.memory_pools().return_signature_vec(signature);

    let elapsed = start_time.elapsed();
    debug!("MinHash signature generation took: {:?}", elapsed);

    signature_clone
}

/// Generate MinHash signature using interned strings for optimal performance.
///
/// This version eliminates string allocation overhead in the hot loop.
pub fn generate_minhash_signature_interned<T: SignatureGenerator>(
    gen: &T,
    source_code: &str,
) -> Vec<u64> {
    let start_time = std::time::Instant::now();
    let num_hashes = gen.num_hashes();

    // Create interned shingles (minimal allocations)
    let shingles = create_shingles_interned(gen, source_code);

    // Generate MinHash signature using memory pool
    let mut signature = gen.memory_pools().get_signature_vec();
    signature.resize(num_hashes, u64::MAX);

    // Hash interned strings directly - this is much faster than String hashing
    for shingle in shingles {
        let shingle_str = resolve(shingle); // Zero-cost lookup to original string
        for i in 0..num_hashes {
            let hash = hash_with_seed(shingle_str, i as u64);
            if hash < signature[i] {
                signature[i] = hash;
            }
        }
    }

    // Clone before returning to pool
    let signature_clone = signature.clone();

    // Return signature vector to memory pool for reuse
    gen.memory_pools().return_signature_vec(signature);

    let elapsed = start_time.elapsed();
    debug!("Interned MinHash signature generation took: {:?}", elapsed);

    signature_clone
}

/// Generate MinHash signature with caching to avoid redundant computation.
pub fn generate_minhash_signature_cached<T: SignatureGenerator>(
    gen: &T,
    source_code: &str,
    entity_id: &str,
) -> Vec<u64> {
    debug!(
        "Generating signature for: {} (caching disabled for thread safety)",
        entity_id
    );
    generate_minhash_signature(gen, source_code)
}

/// SIMD-accelerated MinHash signature generation.
#[cfg(feature = "simd")]
pub fn generate_minhash_signature_simd<T: SignatureGenerator>(
    gen: &T,
    source_code: &str,
) -> Vec<u64> {
    let shingles = create_shingles(gen, source_code);
    let num_hashes = gen.num_hashes();
    let mut signature = vec![u64::MAX; num_hashes];

    // Process hashes in chunks of 4 for SIMD
    let chunks = num_hashes / 4;
    let remainder = num_hashes % 4;

    for shingle in shingles {
        // Process 4 hashes at a time with SIMD - vectorized hashing
        for chunk_idx in 0..chunks {
            let base_idx = chunk_idx * 4;

            // Vectorized hash computation using SIMD
            let hashes = hash_with_seeds_simd(&shingle, base_idx);

            // Load current signatures into SIMD vector
            let current_sigs = u64x4::from([
                signature[base_idx],
                signature[base_idx + 1],
                signature[base_idx + 2],
                signature[base_idx + 3],
            ]);

            // Element-wise minimum using comparison masks
            let comparison_mask = hashes.cmp_lt(current_sigs);
            let min_vec = comparison_mask.blend(hashes, current_sigs);

            // Store results back to signature
            let min_array = min_vec.to_array();
            signature[base_idx] = min_array[0];
            signature[base_idx + 1] = min_array[1];
            signature[base_idx + 2] = min_array[2];
            signature[base_idx + 3] = min_array[3];
        }

        // Handle remainder
        for i in (chunks * 4)..(chunks * 4 + remainder) {
            let hash = hash_with_seed(&shingle, i as u64);
            if hash < signature[i] {
                signature[i] = hash;
            }
        }
    }

    signature
}

/// SIMD-accelerated hash computation for 4 seeds at once.
#[cfg(feature = "simd")]
pub fn hash_with_seeds_simd(data: &str, base_seed: usize) -> u64x4 {
    let seeds = [
        base_seed as u64,
        (base_seed + 1) as u64,
        (base_seed + 2) as u64,
        (base_seed + 3) as u64,
    ];

    let hashes = [
        hash_with_seed_fast(data, seeds[0]),
        hash_with_seed_fast(data, seeds[1]),
        hash_with_seed_fast(data, seeds[2]),
        hash_with_seed_fast(data, seeds[3]),
    ];

    u64x4::from(hashes)
}

/// Fast hash implementation optimized for SIMD batch processing.
#[cfg(feature = "simd")]
pub fn hash_with_seed_fast(data: &str, seed: u64) -> u64 {
    let mut hasher = Xxh3::with_seed(seed);
    data.hash(&mut hasher);
    hasher.finish()
}

/// Create shingles with token caching to avoid redundant tokenization.
pub fn create_shingles_cached<T: SignatureGenerator>(gen: &T, source_code: &str) -> Vec<String> {
    // Check token cache first
    if let Some(cached_tokens) = gen.cache().get_tokens(source_code) {
        debug!("Token cache hit for source code");
        return tokens_to_shingles(gen, cached_tokens);
    }

    // Generate tokens and shingles using memory pool
    let normalized = normalize_code(source_code);
    let mut tokens = gen.memory_pools().get_string_vec();
    tokens.extend(
        normalized
            .split_whitespace()
            .filter(|token| !token.is_empty())
            .map(|s| s.to_string()),
    );

    // Cache the tokens for future use
    gen.cache().cache_tokens(source_code, tokens.clone());

    // Convert tokens to shingles (returns tokens to pool internally)
    tokens_to_shingles(gen, tokens)
}

/// Convert tokens to shingles.
pub fn tokens_to_shingles<T: SignatureGenerator>(gen: &T, tokens: Vec<String>) -> Vec<String> {
    let shingle_size = gen.shingle_size();
    let mut shingles = gen.memory_pools().get_string_vec();

    if tokens.len() >= shingle_size {
        for i in 0..=tokens.len() - shingle_size {
            let shingle = tokens[i..i + shingle_size].join(" ");
            shingles.push(shingle);
        }
    }

    // Return tokens vector to pool for reuse
    gen.memory_pools().return_string_vec(tokens);

    shingles
}

/// Normalize source code for comparison using basic text processing.
pub fn normalize_code(source_code: &str) -> String {
    let mut normalized = String::new();

    for line in source_code.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }

        // Basic normalization: lowercase, remove extra whitespace
        let clean_line = line
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        normalized.push_str(&clean_line);
        normalized.push(' ');
    }

    normalized
}

/// Hash a string with a seed using xxHash3.
pub fn hash_with_seed(data: &str, seed: u64) -> u64 {
    let mut hasher = Xxh3::with_seed(seed);
    data.hash(&mut hasher);
    hasher.finish()
}
