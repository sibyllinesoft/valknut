use super::*;
use super::module_resolver::path_to_rust_module;
use std::fs;
use tempfile::TempDir;

fn create_test_project() -> (TempDir, Vec<PathBuf>) {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a simple project structure
    fs::create_dir_all(root.join("src/core")).unwrap();
    fs::create_dir_all(root.join("src/utils")).unwrap();

    // Main file imports core
    fs::write(
        root.join("src/main.rs"),
        "mod core;\nmod utils;\n\nfn main() { core::run(); }",
    )
    .unwrap();

    // Core module imports utils
    fs::write(
        root.join("src/core/mod.rs"),
        "use crate::utils;\n\npub fn run() { utils::helper(); }",
    )
    .unwrap();

    // Utils is standalone
    fs::write(
        root.join("src/utils/mod.rs"),
        "pub fn helper() { println!(\"helper\"); }",
    )
    .unwrap();

    let files = vec![
        PathBuf::from("src/main.rs"),
        PathBuf::from("src/core/mod.rs"),
        PathBuf::from("src/utils/mod.rs"),
    ];

    (temp_dir, files)
}

#[test]
fn test_partition_config_default() {
    let config = PartitionConfig::default();
    assert_eq!(config.slice_token_budget, 200_000);
    assert_eq!(config.min_files_per_slice, 3);
    assert!(config.allow_overlap);
}

#[test]
fn test_partition_empty_files() {
    let partitioner = ImportGraphPartitioner::default();
    let temp_dir = TempDir::new().unwrap();
    let result = partitioner.partition(temp_dir.path(), &[]).unwrap();

    assert!(result.slices.is_empty());
    assert!(result.unassigned.is_empty());
    assert_eq!(result.stats.total_files, 0);
}

#[test]
fn test_partition_simple_project() {
    let (temp_dir, files) = create_test_project();
    let partitioner = ImportGraphPartitioner::default();

    let result = partitioner.partition(temp_dir.path(), &files).unwrap();

    // Should create at least one slice
    assert!(!result.slices.is_empty());
    // All files should be assigned
    assert!(result.unassigned.is_empty());
    // Stats should be populated
    assert_eq!(result.stats.total_files, 3);
}

#[test]
fn test_partition_respects_token_budget() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    fs::create_dir_all(root.join("src")).unwrap();

    // Create files that together exceed 1000 tokens
    for i in 0..10 {
        let content = format!("fn func_{}() {{\n{}\n}}", i, "    let x = 1;\n".repeat(100));
        fs::write(root.join(format!("src/file_{}.rs", i)), content).unwrap();
    }

    let files: Vec<PathBuf> = (0..10)
        .map(|i| PathBuf::from(format!("src/file_{}.rs", i)))
        .collect();

    let config = PartitionConfig::default().with_token_budget(1000);
    let partitioner = ImportGraphPartitioner::new(config);

    let result = partitioner.partition(root, &files).unwrap();

    // Should create multiple slices due to small budget
    assert!(result.slices.len() > 1);
    // Each slice should respect the budget
    for slice in &result.slices {
        assert!(slice.token_count <= 1000 || slice.files.len() == 1);
    }
}

#[test]
fn test_code_slice_contains() {
    let slice = CodeSlice {
        id: 0,
        files: vec![PathBuf::from("src/main.rs")],
        contents: HashMap::new(),
        token_count: 100,
        bridge_dependencies: vec![PathBuf::from("src/utils.rs")],
        primary_module: None,
    };

    assert!(slice.contains(Path::new("src/main.rs")));
    assert!(slice.contains(Path::new("src/utils.rs")));
    assert!(!slice.contains(Path::new("src/other.rs")));
}

#[test]
fn test_path_to_rust_module() {
    // Test Rust module path conversion
    assert_eq!(path_to_rust_module("src/core/config"), "core::config");
    assert_eq!(
        path_to_rust_module("src/core/pipeline/mod"),
        "core::pipeline::mod"
    );
    assert_eq!(path_to_rust_module("utils/helper"), "utils::helper");
}

#[test]
fn test_directory_similarity() {
    let partitioner = ImportGraphPartitioner::default();

    // Same directory should be 1.0
    let score = partitioner.directory_similarity(
        Path::new("src/core/config.rs"),
        Path::new("src/core/errors.rs"),
    );
    assert!(
        score > 0.9,
        "Same directory should have high similarity: {}",
        score
    );

    // Different subdirectories
    let score = partitioner.directory_similarity(
        Path::new("src/core/config.rs"),
        Path::new("src/io/cache.rs"),
    );
    assert!(
        score > 0.0 && score < 1.0,
        "Different subdirs should have partial similarity: {}",
        score
    );

    // Completely different paths
    let score = partitioner.directory_similarity(
        Path::new("src/core/config.rs"),
        Path::new("tests/unit/test.rs"),
    );
    assert!(
        score < 0.5,
        "Different roots should have low similarity: {}",
        score
    );
}
