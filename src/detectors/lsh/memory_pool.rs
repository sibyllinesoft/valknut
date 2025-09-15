//! Memory pool for reducing allocation churn in LSH operations
//!
//! This module provides memory pools for frequently allocated objects
//! to reduce GC pressure and improve performance in hot paths.

use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use tracing::debug;

/// Memory pool for reusing Vec<String> allocations (for shingles)
#[derive(Debug, Clone)]
pub struct StringVecPool {
    pool: Arc<Mutex<VecDeque<Vec<String>>>>,
    max_size: usize,
    created_count: Arc<Mutex<usize>>,
    reused_count: Arc<Mutex<usize>>,
}

impl StringVecPool {
    /// Create a new string vector pool
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
            created_count: Arc::new(Mutex::new(0)),
            reused_count: Arc::new(Mutex::new(0)),
        }
    }
    
    /// Get a Vec<String> from the pool or create a new one
    pub fn get(&self) -> Vec<String> {
        if let Ok(mut pool) = self.pool.lock() {
            if let Some(mut vec) = pool.pop_front() {
                vec.clear(); // Clear but keep capacity
                if let Ok(mut count) = self.reused_count.lock() {
                    *count += 1;
                }
                debug!("Reused String vector from pool");
                return vec;
            }
        }
        
        // Create new vector if pool is empty
        if let Ok(mut count) = self.created_count.lock() {
            *count += 1;
        }
        debug!("Created new String vector");
        Vec::new()
    }
    
    /// Return a Vec<String> to the pool
    pub fn return_vec(&self, vec: Vec<String>) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size {
                pool.push_back(vec);
                debug!("Returned String vector to pool");
            } else {
                debug!("Pool full, dropping String vector");
            }
        }
    }
    
    /// Get pool statistics
    pub fn get_statistics(&self) -> PoolStatistics {
        let created = self.created_count.lock().map(|c| *c).unwrap_or(0);
        let reused = self.reused_count.lock().map(|c| *c).unwrap_or(0);
        let pool_size = self.pool.lock().map(|p| p.len()).unwrap_or(0);
        
        PoolStatistics {
            created_count: created,
            reused_count: reused,
            current_pool_size: pool_size,
            max_pool_size: self.max_size,
        }
    }
}

/// Memory pool for reusing Vec<u64> allocations (for signatures)
#[derive(Debug, Clone)]
pub struct U64VecPool {
    pool: Arc<Mutex<VecDeque<Vec<u64>>>>,
    max_size: usize,
    signature_size: usize,
    created_count: Arc<Mutex<usize>>,
    reused_count: Arc<Mutex<usize>>,
}

impl U64VecPool {
    /// Create a new u64 vector pool
    pub fn new(max_size: usize, signature_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(VecDeque::with_capacity(max_size))),
            max_size,
            signature_size,
            created_count: Arc::new(Mutex::new(0)),
            reused_count: Arc::new(Mutex::new(0)),
        }
    }
    
    /// Get a Vec<u64> from the pool or create a new one
    pub fn get(&self) -> Vec<u64> {
        if let Ok(mut pool) = self.pool.lock() {
            if let Some(mut vec) = pool.pop_front() {
                vec.clear();
                vec.resize(self.signature_size, u64::MAX); // Pre-fill with MAX values
                if let Ok(mut count) = self.reused_count.lock() {
                    *count += 1;
                }
                debug!("Reused u64 vector from pool");
                return vec;
            }
        }
        
        // Create new vector if pool is empty
        let mut vec = Vec::with_capacity(self.signature_size);
        vec.resize(self.signature_size, u64::MAX);
        
        if let Ok(mut count) = self.created_count.lock() {
            *count += 1;
        }
        debug!("Created new u64 vector");
        vec
    }
    
    /// Return a Vec<u64> to the pool
    pub fn return_vec(&self, vec: Vec<u64>) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size && vec.capacity() >= self.signature_size {
                pool.push_back(vec);
                debug!("Returned u64 vector to pool");
            } else {
                debug!("Pool full or wrong size, dropping u64 vector");
            }
        }
    }
    
    /// Get pool statistics
    pub fn get_statistics(&self) -> PoolStatistics {
        let created = self.created_count.lock().map(|c| *c).unwrap_or(0);
        let reused = self.reused_count.lock().map(|c| *c).unwrap_or(0);
        let pool_size = self.pool.lock().map(|p| p.len()).unwrap_or(0);
        
        PoolStatistics {
            created_count: created,
            reused_count: reused,
            current_pool_size: pool_size,
            max_pool_size: self.max_size,
        }
    }
}

/// Statistics for memory pool usage
#[derive(Debug, Clone)]
pub struct PoolStatistics {
    pub created_count: usize,
    pub reused_count: usize,
    pub current_pool_size: usize,
    pub max_pool_size: usize,
}

impl PoolStatistics {
    /// Calculate reuse rate as a percentage
    pub fn reuse_rate(&self) -> f64 {
        let total = self.created_count + self.reused_count;
        if total == 0 {
            0.0
        } else {
            self.reused_count as f64 / total as f64
        }
    }
    
    /// Calculate pool utilization
    pub fn utilization(&self) -> f64 {
        if self.max_pool_size == 0 {
            0.0
        } else {
            self.current_pool_size as f64 / self.max_pool_size as f64
        }
    }
}

/// Combined memory pools for LSH operations
#[derive(Debug, Clone)]
pub struct LshMemoryPools {
    string_pool: StringVecPool,
    signature_pool: U64VecPool,
}

impl LshMemoryPools {
    /// Create new memory pools with default sizes
    pub fn new() -> Self {
        Self::with_capacity(50, 128) // 50 vectors max, 128-element signatures
    }
    
    /// Create memory pools with specified capacities
    pub fn with_capacity(max_vectors: usize, signature_size: usize) -> Self {
        Self {
            string_pool: StringVecPool::new(max_vectors),
            signature_pool: U64VecPool::new(max_vectors, signature_size),
        }
    }
    
    /// Get a string vector for shingles
    pub fn get_string_vec(&self) -> Vec<String> {
        self.string_pool.get()
    }
    
    /// Return a string vector to the pool
    pub fn return_string_vec(&self, vec: Vec<String>) {
        self.string_pool.return_vec(vec);
    }
    
    /// Get a u64 vector for signatures
    pub fn get_signature_vec(&self) -> Vec<u64> {
        self.signature_pool.get()
    }
    
    /// Return a u64 vector to the pool
    pub fn return_signature_vec(&self, vec: Vec<u64>) {
        self.signature_pool.return_vec(vec);
    }
    
    /// Get combined statistics
    pub fn get_statistics(&self) -> (PoolStatistics, PoolStatistics) {
        (self.string_pool.get_statistics(), self.signature_pool.get_statistics())
    }
    
    /// Log pool statistics
    pub fn log_statistics(&self) {
        let (string_stats, sig_stats) = self.get_statistics();
        
        debug!("String Pool Stats: created={}, reused={}, utilization={:.1}%, reuse_rate={:.1}%",
               string_stats.created_count,
               string_stats.reused_count,
               string_stats.utilization() * 100.0,
               string_stats.reuse_rate() * 100.0);
        
        debug!("Signature Pool Stats: created={}, reused={}, utilization={:.1}%, reuse_rate={:.1}%",
               sig_stats.created_count,
               sig_stats.reused_count,
               sig_stats.utilization() * 100.0,
               sig_stats.reuse_rate() * 100.0);
    }
}

impl Default for LshMemoryPools {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_string_vec_pool() {
        let pool = StringVecPool::new(5);
        
        // Get a vector from empty pool (should create new)
        let vec1 = pool.get();
        assert_eq!(vec1.len(), 0);
        
        // Modify and return vector
        let mut vec1_modified = vec1;
        vec1_modified.push("test".to_string());
        vec1_modified.push("string".to_string());
        pool.return_vec(vec1_modified);
        
        // Get another vector (should reuse)
        let vec2 = pool.get();
        assert_eq!(vec2.len(), 0); // Should be cleared
        assert!(vec2.capacity() > 0); // Should retain capacity
        
        let stats = pool.get_statistics();
        assert_eq!(stats.created_count, 1);
        assert_eq!(stats.reused_count, 1);
        assert_eq!(stats.reuse_rate(), 0.5);
    }
    
    #[test]
    fn test_u64_vec_pool() {
        let pool = U64VecPool::new(3, 64);
        
        // Get vector from empty pool
        let vec1 = pool.get();
        assert_eq!(vec1.len(), 64);
        assert!(vec1.iter().all(|&x| x == u64::MAX));
        
        // Modify and return
        let mut vec1_modified = vec1;
        vec1_modified[0] = 42;
        vec1_modified[1] = 123;
        pool.return_vec(vec1_modified);
        
        // Get again (should be reused and reset)
        let vec2 = pool.get();
        assert_eq!(vec2.len(), 64);
        assert!(vec2.iter().all(|&x| x == u64::MAX));
        
        let stats = pool.get_statistics();
        assert!(stats.reused_count > 0);
    }
    
    #[test]
    fn test_pool_size_limits() {
        let pool = StringVecPool::new(2); // Very small pool
        
        // Fill pool beyond capacity
        let vec1 = pool.get();
        let vec2 = pool.get();
        let vec3 = pool.get();
        
        pool.return_vec(vec1);
        pool.return_vec(vec2);
        pool.return_vec(vec3); // This should be dropped
        
        let stats = pool.get_statistics();
        assert!(stats.current_pool_size <= 2, "Pool should not exceed max size");
    }
    
    #[test]
    fn test_lsh_memory_pools() {
        let pools = LshMemoryPools::with_capacity(10, 32);
        
        // Test string vector operations
        let mut string_vec = pools.get_string_vec();
        string_vec.push("test".to_string());
        pools.return_string_vec(string_vec);
        
        // Test signature vector operations
        let mut sig_vec = pools.get_signature_vec();
        sig_vec[0] = 12345;
        pools.return_signature_vec(sig_vec);
        
        // Verify reuse
        let reused_string = pools.get_string_vec();
        let reused_sig = pools.get_signature_vec();
        
        assert_eq!(reused_string.len(), 0); // Should be cleared
        assert_eq!(reused_sig.len(), 32); // Should be reset to MAX values
        assert_eq!(reused_sig[0], u64::MAX); // Should be reset
        
        let (string_stats, sig_stats) = pools.get_statistics();
        assert!(string_stats.reused_count > 0);
        assert!(sig_stats.reused_count > 0);
    }
}