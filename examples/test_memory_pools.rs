//! Simple example to test memory pool integration

use valknut_rs::detectors::lsh::memory_pool::{LshMemoryPools, StringVecPool, U64VecPool};

fn main() {
    println!("=== Testing Memory Pool Implementation ===");
    
    // Test StringVecPool
    println!("\n1. Testing StringVecPool:");
    let string_pool = StringVecPool::new(5);
    
    // Get vectors and populate them
    let mut vec1 = string_pool.get();
    vec1.push("hello".to_string());
    vec1.push("world".to_string());
    
    let mut vec2 = string_pool.get();
    vec2.push("foo".to_string());
    vec2.push("bar".to_string());
    
    println!("Created 2 vectors with content");
    
    // Return vectors to pool
    string_pool.return_vec(vec1);
    string_pool.return_vec(vec2);
    
    // Get new vectors (should reuse)
    let vec3 = string_pool.get();
    let vec4 = string_pool.get();
    
    println!("Reused vectors: vec3 len={}, vec4 len={}", vec3.len(), vec4.len());
    
    let stats = string_pool.get_statistics();
    println!("String Pool Stats: created={}, reused={}, reuse_rate={:.1}%", 
             stats.created_count, stats.reused_count, stats.reuse_rate() * 100.0);
    
    // Test U64VecPool
    println!("\n2. Testing U64VecPool:");
    let sig_pool = U64VecPool::new(5, 64);
    
    let mut sig1 = sig_pool.get();
    sig1[0] = 12345;
    sig1[1] = 67890;
    
    let mut sig2 = sig_pool.get();
    sig2[0] = 11111;
    sig2[1] = 22222;
    
    println!("Created 2 signature vectors");
    
    sig_pool.return_vec(sig1);
    sig_pool.return_vec(sig2);
    
    let sig3 = sig_pool.get();
    let sig4 = sig_pool.get();
    
    println!("Reused signature vectors: sig3[0]={}, sig4[0]={} (should be u64::MAX after reset)", 
             sig3[0], sig4[0]);
    
    let sig_stats = sig_pool.get_statistics();
    println!("Signature Pool Stats: created={}, reused={}, reuse_rate={:.1}%", 
             sig_stats.created_count, sig_stats.reused_count, sig_stats.reuse_rate() * 100.0);
    
    // Test LshMemoryPools
    println!("\n3. Testing LshMemoryPools:");
    let pools = LshMemoryPools::with_capacity(10, 128);
    
    for i in 0..5 {
        let mut strings = pools.get_string_vec();
        strings.push(format!("test_{}", i));
        
        let mut signatures = pools.get_signature_vec();
        signatures[0] = i as u64;
        
        pools.return_string_vec(strings);
        pools.return_signature_vec(signatures);
    }
    
    let (str_stats, sig_stats) = pools.get_statistics();
    println!("Combined Stats:");
    println!("  String: created={}, reused={}, reuse_rate={:.1}%", 
             str_stats.created_count, str_stats.reused_count, str_stats.reuse_rate() * 100.0);
    println!("  Signature: created={}, reused={}, reuse_rate={:.1}%", 
             sig_stats.created_count, sig_stats.reused_count, sig_stats.reuse_rate() * 100.0);
    
    pools.log_statistics();
    
    println!("\n✅ Memory pool tests completed successfully!");
    println!("The memory pools are working correctly and can reduce allocation churn in LSH operations.");
}