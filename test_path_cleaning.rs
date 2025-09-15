#!/usr/bin/env rust-script

// Simple script to demonstrate the path cleaning functionality
use std::path::PathBuf;

fn clean_path_prefix(path: &str) -> String {
    if path.starts_with("./") {
        path[2..].to_string()
    } else {
        path.to_string()
    }
}

fn main() {
    println!("Path Cleaning Demonstration:");
    println!("============================");
    
    let test_paths = vec![
        "./src/core/config.rs",
        "./src/api/engine.rs", 
        "./benches/performance.rs",
        "src/lib.rs", // already clean
        "./",
        ".",
    ];
    
    for path in test_paths {
        let cleaned = clean_path_prefix(path);
        println!("Original: '{}' -> Cleaned: '{}'", path, cleaned);
    }
    
    println!("\nThis demonstrates how directory health tree paths will be cleaned:");
    println!("  Before: {'./src': {...}, './src/core': {...}}");
    println!("  After:  {'src': {...}, 'src/core': {...}}");
}