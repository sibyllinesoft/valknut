use std::fs;
use std::time::Duration;

use anyhow::Result;
use tempfile::tempdir;
use valknut_rs::api::config_types::AnalysisConfig;
use valknut_rs::api::engine::ValknutEngine;
use valknut_rs::core::config::ValknutConfig;
use valknut_rs::core::pipeline::{AnalysisConfig as PipelineAnalysisConfig, AnalysisPipeline};

fn create_sample_project() -> Result<tempfile::TempDir> {
    let project = tempdir()?;
    let root = project.path();

    // Python module with deliberate complexity
    fs::write(
        root.join("analytics.py"),
        r#"
def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n - 1) + fibonacci(n - 2)

class Analyzer:
    def __init__(self, values):
        self.values = values

    def average(self):
        total = sum(self.values)
        return total / len(self.values)
"#,
    )?;

    // Rust module
    fs::create_dir_all(root.join("rust_mod"))?;
    fs::write(
        root.join("rust_mod/lib.rs"),
        r#"
/// Compute factorial with simple recursion.
pub fn factorial(n: u64) -> u64 {
    match n {
        0 | 1 => 1,
        _ => n * factorial(n - 1),
    }
}

pub struct Accumulator {
    total: i64,
}

impl Accumulator {
    pub fn new() -> Self {
        Self { total: 0 }
    }

    pub fn add(&mut self, value: i64) {
        self.total += value;
    }

    pub fn total(&self) -> i64 {
        self.total
    }
}
"#,
    )?;

    // TypeScript module
    fs::write(
        root.join("metrics.ts"),
        r#"
export function mean(values: number[]): number {
    if (values.length === 0) {
        return 0;
    }
    return values.reduce((sum, value) => sum + value, 0) / values.length;
}

export class MovingAverage {
    private window: number[];
    constructor(initial: number[]) {
        this.window = initial.slice();
    }

    push(value: number) {
        this.window.push(value);
        if (this.window.length > 5) {
            this.window.shift();
        }
    }

    value(): number {
        return mean(this.window);
    }
}
"#,
    )?;

    Ok(project)
}

fn create_lsh_and_coverage_project() -> Result<tempfile::TempDir> {
    let project = tempdir()?;
    let root = project.path();

    fs::create_dir_all(root.join("src"))?;
    fs::write(
        root.join("src/lib.rs"),
        r#"
pub fn duplicate_one(value: i32) -> i32 {
    if value > 0 {
        value + 1
    } else {
        value - 1
    }
}

pub fn duplicate_two(value: i32) -> i32 {
    if value > 0 {
        value + 1
    } else {
        value - 1
    }
}
"#,
    )?;

    let coverage_dir = root.join("coverage");
    fs::create_dir_all(&coverage_dir)?;
    fs::write(
        coverage_dir.join("coverage.lcov"),
        "TN:valknut-test\nSF:src/lib.rs\nFN:2,duplicate_one\nFN:9,duplicate_two\nFNF:2\nFNH:2\nFNDA:3,duplicate_one\nFNDA:3,duplicate_two\nDA:2,3\nDA:3,3\nDA:9,3\nDA:10,3\nLF:4\nLH:4\nend_of_record\n",
    )?;

    Ok(project)
}

#[tokio::test]
async fn pipeline_enables_lsh_and_coverage_analysis() -> Result<()> {
    let project = create_lsh_and_coverage_project()?;
    let root = project.path().to_path_buf();

    let mut valknut_config = ValknutConfig::default();
    valknut_config.analysis.enable_lsh_analysis = true;
    valknut_config.analysis.enable_coverage_analysis = true;
    valknut_config.denoise.enabled = true;
    valknut_config.denoise.min_function_tokens = 1;
    valknut_config.denoise.min_match_tokens = 1;
    valknut_config.denoise.require_blocks = 1;
    valknut_config.dedupe.min_function_tokens = 1;
    valknut_config.dedupe.min_ast_nodes = 1;
    valknut_config.dedupe.min_match_tokens = 1;
    valknut_config.coverage.search_paths = vec!["./coverage/".to_string(), "./".to_string()];
    valknut_config.coverage.file_patterns = vec!["coverage.lcov".to_string()];

    let mut pipeline_config = PipelineAnalysisConfig::from(valknut_config.clone());
    pipeline_config.enable_lsh_analysis = true;
    pipeline_config.enable_coverage_analysis = true;
    pipeline_config.file_extensions = vec!["rs".to_string()];

    let pipeline = AnalysisPipeline::new_with_config(pipeline_config, valknut_config);
    let results = pipeline
        .analyze_paths(&[root], None)
        .await
        .expect("pipeline execution should succeed");

    assert!(results.lsh.enabled, "LSH analysis should be enabled");
    assert!(
        results.lsh.denoising_enabled,
        "Denoising should be reflected in LSH results"
    );
    assert!(results.coverage.enabled, "Coverage analysis should run");
    assert!(
        !results.coverage.coverage_files_used.is_empty(),
        "Coverage discovery should pick up the LCOV file"
    );

    Ok(())
}

#[tokio::test]
async fn full_pipeline_smoke_test_covers_key_modules() -> Result<()> {
    let project = create_sample_project()?;

    let config = AnalysisConfig::new()
        .modules(|mut modules| {
            modules.duplicates = true;
            modules.coverage = false;
            modules
        })
        .languages(|mut languages| {
            languages.enabled = vec![
                "python".to_string(),
                "rust".to_string(),
                "typescript".to_string(),
            ];
            languages
        })
        .files(|mut files| {
            files.exclude_patterns.clear();
            files
        });

    let mut engine = ValknutEngine::new(config).await?;

    let results = engine.analyze_directory(project.path()).await?;
    assert!(
        results.files_analyzed() >= 3,
        "expected multiple files analyzed"
    );
    assert!(
        results.summary.entities_analyzed > 0,
        "expected entities to be analyzed"
    );

    // Exercise file analysis path
    let files = [
        project.path().join("analytics.py"),
        project.path().join("metrics.ts"),
    ];
    let file_results = engine.analyze_files(&files).await?;
    assert!(
        file_results.files_analyzed() >= 2,
        "expected per-file analysis to run"
    );

    // Health check touches configuration validation and status paths
    let health = engine.health_check().await;
    assert!(health.overall_status);
    assert!(!health.checks.is_empty());

    // Allow async tasks to flush logs before tempdir drops in case of CI latency
    tokio::time::sleep(Duration::from_millis(50)).await;

    Ok(())
}
