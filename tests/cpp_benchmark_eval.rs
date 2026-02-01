//! Benchmark evaluation for C++ adapter against real-world codebases.
//!
//! Run with: cargo test --test cpp_benchmark_eval -- --nocapture

use std::fs;
use std::path::Path;
use valknut_rs::lang::{CppAdapter, LanguageAdapter};
use walkdir::WalkDir;

/// C++ file extensions to test
const CPP_EXTENSIONS: &[&str] = &["cpp", "cxx", "cc", "c++", "hpp", "hxx", "hh", "h++", "h"];

/// Evaluate a single repository
fn evaluate_repo(repo_path: &Path) -> (usize, usize, usize, Vec<String>) {
    let mut total = 0;
    let mut success = 0;
    let mut entity_count = 0;
    let mut errors = Vec::new();

    let mut adapter = match CppAdapter::new() {
        Ok(a) => a,
        Err(e) => {
            errors.push(format!("Failed to create adapter: {}", e));
            return (0, 0, 0, errors);
        }
    };

    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if CPP_EXTENSIONS.contains(&ext_str.as_str()) {
                total += 1;

                // Skip very large files (> 500KB) as they may be generated
                if let Ok(metadata) = fs::metadata(path) {
                    if metadata.len() > 500_000 {
                        continue;
                    }
                }

                match fs::read_to_string(path) {
                    Ok(content) => {
                        let path_str = path.to_string_lossy();
                        match adapter.parse_source(&content, &path_str) {
                            Ok(index) => {
                                success += 1;
                                entity_count += index.entities.len();
                            }
                            Err(e) => {
                                // Limit error collection
                                if errors.len() < 50 {
                                    errors.push(format!("{}: {}", path.display(), e));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Skip files with encoding issues
                        total -= 1;
                    }
                }
            }
        }
    }

    (total, success, entity_count, errors)
}

#[test]
fn benchmark_fmt() {
    let repo_path = Path::new("benchmarks/cpp/fmt");
    if !repo_path.exists() {
        println!("Skipping fmt - not cloned");
        return;
    }

    let (total, success, entities, errors) = evaluate_repo(repo_path);
    println!("\n=== fmt ===");
    println!("Files: {}, Parsed: {}, Entities: {}", total, success, entities);

    if !errors.is_empty() {
        println!("Errors ({}):", errors.len());
        for e in errors.iter().take(10) {
            println!("  {}", e);
        }
    }

    let rate = if total > 0 { success * 100 / total } else { 0 };
    println!("Success rate: {}%", rate);
    assert!(rate >= 90, "fmt success rate should be >= 90%");
}

#[test]
fn benchmark_json() {
    let repo_path = Path::new("benchmarks/cpp/json");
    if !repo_path.exists() {
        println!("Skipping json - not cloned");
        return;
    }

    let (total, success, entities, errors) = evaluate_repo(repo_path);
    println!("\n=== nlohmann/json ===");
    println!("Files: {}, Parsed: {}, Entities: {}", total, success, entities);

    if !errors.is_empty() {
        println!("Errors ({}):", errors.len());
        for e in errors.iter().take(10) {
            println!("  {}", e);
        }
    }

    let rate = if total > 0 { success * 100 / total } else { 0 };
    println!("Success rate: {}%", rate);
    assert!(rate >= 90, "json success rate should be >= 90%");
}

#[test]
fn benchmark_spdlog() {
    let repo_path = Path::new("benchmarks/cpp/spdlog");
    if !repo_path.exists() {
        println!("Skipping spdlog - not cloned");
        return;
    }

    let (total, success, entities, errors) = evaluate_repo(repo_path);
    println!("\n=== spdlog ===");
    println!("Files: {}, Parsed: {}, Entities: {}", total, success, entities);

    if !errors.is_empty() {
        println!("Errors ({}):", errors.len());
        for e in errors.iter().take(10) {
            println!("  {}", e);
        }
    }

    let rate = if total > 0 { success * 100 / total } else { 0 };
    println!("Success rate: {}%", rate);
    assert!(rate >= 90, "spdlog success rate should be >= 90%");
}

#[test]
fn benchmark_googletest() {
    let repo_path = Path::new("benchmarks/cpp/googletest");
    if !repo_path.exists() {
        println!("Skipping googletest - not cloned");
        return;
    }

    let (total, success, entities, errors) = evaluate_repo(repo_path);
    println!("\n=== googletest ===");
    println!("Files: {}, Parsed: {}, Entities: {}", total, success, entities);

    if !errors.is_empty() {
        println!("Errors ({}):", errors.len());
        for e in errors.iter().take(10) {
            println!("  {}", e);
        }
    }

    let rate = if total > 0 { success * 100 / total } else { 0 };
    println!("Success rate: {}%", rate);
    assert!(rate >= 90, "googletest success rate should be >= 90%");
}

#[test]
fn benchmark_cpr() {
    let repo_path = Path::new("benchmarks/cpp/cpr");
    if !repo_path.exists() {
        println!("Skipping cpr - not cloned");
        return;
    }

    let (total, success, entities, errors) = evaluate_repo(repo_path);
    println!("\n=== cpr ===");
    println!("Files: {}, Parsed: {}, Entities: {}", total, success, entities);

    if !errors.is_empty() {
        println!("Errors ({}):", errors.len());
        for e in errors.iter().take(10) {
            println!("  {}", e);
        }
    }

    let rate = if total > 0 { success * 100 / total } else { 0 };
    println!("Success rate: {}%", rate);
    assert!(rate >= 90, "cpr success rate should be >= 90%");
}

/// Detailed failure analysis - finds files that don't parse successfully
#[test]
fn analyze_failures() {
    println!("\n====== C++ Adapter Failure Analysis ======\n");

    // Only check repos that had issues
    let repos = [
        ("fmt", "benchmarks/cpp/fmt"),
        ("json", "benchmarks/cpp/json"),
    ];

    let mut all_failures: Vec<(String, String, String)> = Vec::new();

    for (name, path_str) in repos.iter() {
        let repo_path = Path::new(path_str);
        if !repo_path.exists() {
            continue;
        }

        let mut adapter = CppAdapter::new().unwrap();

        for entry in WalkDir::new(repo_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if CPP_EXTENSIONS.contains(&ext_str.as_str()) {
                    // Skip large files
                    if let Ok(metadata) = fs::metadata(path) {
                        if metadata.len() > 500_000 {
                            continue;
                        }
                    }

                    match fs::read_to_string(path) {
                        Ok(content) => {
                            let path_str = path.to_string_lossy();
                            match adapter.parse_source(&content, &path_str) {
                                Ok(index) => {
                                    // Check if we got 0 entities from a non-empty file
                                    if index.entities.is_empty() && content.len() > 100 {
                                        // File parsed but no entities - might be worth checking
                                        all_failures.push((
                                            name.to_string(),
                                            path.display().to_string(),
                                            format!("0 entities from {} bytes", content.len()),
                                        ));
                                    }
                                }
                                Err(e) => {
                                    all_failures.push((
                                        name.to_string(),
                                        path.display().to_string(),
                                        e.to_string(),
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            all_failures.push((
                                name.to_string(),
                                path.display().to_string(),
                                format!("Read error: {}", e),
                            ));
                        }
                    }
                }
            }
        }
    }

    println!("Total issues found: {}\n", all_failures.len());

    // Group by error type
    let mut parse_errors = Vec::new();
    let mut zero_entity = Vec::new();
    let mut read_errors = Vec::new();

    for (repo, path, error) in all_failures.iter() {
        if error.starts_with("Read error") {
            read_errors.push((repo, path, error));
        } else if error.contains("0 entities") {
            zero_entity.push((repo, path, error));
        } else {
            parse_errors.push((repo, path, error));
        }
    }

    if !parse_errors.is_empty() {
        println!("=== Parse Errors ({}) ===", parse_errors.len());
        for (repo, path, error) in parse_errors.iter().take(20) {
            println!("[{}] {}\n  Error: {}\n", repo, path, error);
        }
    }

    if !read_errors.is_empty() {
        println!("=== Read Errors ({}) ===", read_errors.len());
        for (repo, path, error) in read_errors.iter().take(10) {
            println!("[{}] {}: {}", repo, path, error);
        }
    }

    if !zero_entity.is_empty() {
        println!("\n=== Zero Entities ({}) ===", zero_entity.len());
        for (repo, path, error) in zero_entity.iter().take(20) {
            println!("[{}] {}: {}", repo, path, error);
        }
    }
}

/// Summary test that runs all available benchmarks
#[test]
fn benchmark_summary() {
    println!("\n====== C++ Adapter Benchmark Summary ======\n");

    let repos = [
        // Phase 1: Smoke Tests
        ("fmt", "benchmarks/cpp/fmt"),
        ("json", "benchmarks/cpp/json"),
        ("spdlog", "benchmarks/cpp/spdlog"),
        ("googletest", "benchmarks/cpp/googletest"),
        ("cpr", "benchmarks/cpp/cpr"),
        // Phase 2: Stress Tests
        ("simdjson", "benchmarks/cpp/simdjson"),
        ("catch2", "benchmarks/cpp/catch2"),
        ("range-v3", "benchmarks/cpp/range-v3"),
        ("entt", "benchmarks/cpp/entt"),
        ("beast", "benchmarks/cpp/beast"),
        ("abseil-cpp", "benchmarks/cpp/abseil-cpp"),
        ("folly", "benchmarks/cpp/folly"),
        ("drogon", "benchmarks/cpp/drogon"),
        ("oatpp", "benchmarks/cpp/oatpp"),
        ("grpc", "benchmarks/cpp/grpc"),
        // Phase 3: Full Validation
        ("protobuf", "benchmarks/cpp/protobuf"),
        ("godot", "benchmarks/cpp/godot"),
        ("acid", "benchmarks/cpp/acid"),
        ("halley", "benchmarks/cpp/halley"),
        ("rocksdb", "benchmarks/cpp/rocksdb"),
        ("clickhouse", "benchmarks/cpp/clickhouse"),
        ("opencv", "benchmarks/cpp/opencv"),
        ("tensorflow", "benchmarks/cpp/tensorflow"),
        ("etl", "benchmarks/cpp/etl"),
        ("zephyr", "benchmarks/cpp/zephyr"),
        // Phase 4: Exotic/Stress Test
        ("hana", "benchmarks/cpp/hana"),
        ("spirit", "benchmarks/cpp/spirit"),
        ("metal", "benchmarks/cpp/metal"),
        ("dolphin", "benchmarks/cpp/dolphin"),
        ("duckstation", "benchmarks/cpp/duckstation"),
        ("mame", "benchmarks/cpp/mame"),
        ("ppsspp", "benchmarks/cpp/ppsspp"),
        ("arduino-esp32", "benchmarks/cpp/arduino-esp32"),
        ("serenity", "benchmarks/cpp/serenity"),
        ("contour", "benchmarks/cpp/contour"),
        ("luajit", "benchmarks/cpp/luajit"),
        ("mono", "benchmarks/cpp/mono"),
        ("filament", "benchmarks/cpp/filament"),
        ("bgfx", "benchmarks/cpp/bgfx"),
        ("magnum", "benchmarks/cpp/magnum"),
        ("obs-studio", "benchmarks/cpp/obs-studio"),
        ("audacity", "benchmarks/cpp/audacity"),
        ("kodi", "benchmarks/cpp/kodi"),
    ];

    let mut total_files = 0;
    let mut total_success = 0;
    let mut total_entities = 0;

    for (name, path_str) in repos.iter() {
        let repo_path = Path::new(path_str);
        if !repo_path.exists() {
            println!("{:15} [not cloned]", name);
            continue;
        }

        let (files, success, entities, errors) = evaluate_repo(repo_path);
        let rate = if files > 0 { success * 100 / files } else { 0 };

        total_files += files;
        total_success += success;
        total_entities += entities;

        let status = if rate >= 95 {
            "✓"
        } else if rate >= 80 {
            "~"
        } else {
            "✗"
        };

        println!(
            "{:15} {} {:4} files, {:4} parsed ({:3}%), {:5} entities",
            name, status, files, success, rate, entities
        );

        if !errors.is_empty() && rate < 95 {
            println!("    First error: {}", errors[0]);
        }
    }

    println!("\n--------------------------------------------");
    let overall_rate = if total_files > 0 {
        total_success * 100 / total_files
    } else {
        0
    };
    println!(
        "TOTAL          {:4} files, {:4} parsed ({:3}%), {:5} entities",
        total_files, total_success, overall_rate, total_entities
    );
    println!("==============================================\n");
}
