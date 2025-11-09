use std::path::PathBuf;

use anyhow::Result;
use serde::Deserialize;
use valknut_rs::core::config::{AnalysisConfig as CoreAnalysisConfig, ValknutConfig};
use valknut_rs::core::pipeline::CloneVerificationResults;
use valknut_rs::core::pipeline::{AnalysisConfig as PipelineAnalysisConfig, AnalysisPipeline};

#[derive(Debug, Deserialize, Clone)]
struct CloneEndpoint {
    name: String,
    path: String,
}

#[derive(Debug, Deserialize, Clone)]
struct CloneVerificationDetail {
    #[serde(default)]
    similarity: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
struct ClonePairReport {
    source: CloneEndpoint,
    target: CloneEndpoint,
    similarity: f64,
    #[serde(default)]
    verification: Option<CloneVerificationDetail>,
}

fn build_configs(verify_with_apted: bool) -> (PipelineAnalysisConfig, ValknutConfig) {
    let mut pipeline_config = PipelineAnalysisConfig::default();
    pipeline_config.enable_lsh_analysis = true;
    pipeline_config.enable_structure_analysis = false;
    pipeline_config.enable_complexity_analysis = false;
    pipeline_config.enable_refactoring_analysis = false;
    pipeline_config.enable_impact_analysis = false;
    pipeline_config.enable_coverage_analysis = false;

    let mut valknut_config = ValknutConfig::default();
    let mut analysis_modules = CoreAnalysisConfig::default();
    analysis_modules.enable_lsh_analysis = true;
    analysis_modules.enable_structure_analysis = false;
    analysis_modules.enable_refactoring_analysis = false;
    analysis_modules.enable_coverage_analysis = false;
    analysis_modules.enable_scoring = false;
    analysis_modules.enable_graph_analysis = false;
    analysis_modules.enable_names_analysis = false;
    valknut_config.analysis = analysis_modules;
    valknut_config.denoise.enabled = false;
    valknut_config.lsh.verify_with_apted = verify_with_apted;

    (pipeline_config, valknut_config)
}

fn format_verification(summary: Option<&CloneVerificationResults>) -> String {
    match summary {
        Some(info) => match info.avg_similarity {
            Some(avg) => format!(
                "method={} scored {}/{} pairs ({}) avg={:.3}",
                info.method, info.pairs_scored, info.pairs_evaluated, info.pairs_considered, avg
            ),
            None => format!(
                "method={} scored {}/{} pairs ({})",
                info.method, info.pairs_scored, info.pairs_evaluated, info.pairs_considered
            ),
        },
        None => "disabled".to_string(),
    }
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let Some(target_dir) = args.next() else {
        anyhow::bail!(
            "Usage: cargo run --release --example clone_verification -- <path> [apted|baseline]"
        );
    };
    let path = PathBuf::from(target_dir);
    let verify_with_apted = match args.next().as_deref() {
        None => true,
        Some("apted") | Some("verify") => true,
        Some("baseline") | Some("no-apted") | Some("disable") => false,
        Some(other) => anyhow::bail!("Unknown mode: {} (expected 'apted' or 'baseline')", other),
    };

    let (analysis_config, valknut_config) = build_configs(verify_with_apted);
    let pipeline = AnalysisPipeline::new_with_config(analysis_config, valknut_config);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime");

    let result = runtime.block_on(async { pipeline.analyze_paths(&[path], None).await })?;

    let clone_results = &result.lsh;
    println!(
        "LSH enabled: {} | clone pairs: {} | avg similarity: {:.4} | max similarity: {:.4}",
        clone_results.enabled,
        clone_results.clone_pairs.len(),
        clone_results.avg_similarity,
        clone_results.max_similarity
    );
    println!(
        "Verification summary: {}",
        format_verification(clone_results.verification.as_ref())
    );

    if clone_results.clone_pairs.is_empty() {
        println!("No clone pairs reported.");
        return Ok(());
    }

    let pairs: Vec<ClonePairReport> = clone_results
        .clone_pairs
        .iter()
        .filter_map(|value| serde_json::from_value::<ClonePairReport>(value.clone()).ok())
        .collect();

    let scored = pairs
        .iter()
        .filter_map(|pair| pair.verification.as_ref()?.similarity)
        .collect::<Vec<_>>();

    println!(
        "Pairs with structural scores: {} / {}",
        scored.len(),
        pairs.len()
    );

    if !pairs.is_empty() {
        println!("Top 5 clone pairs:");
        for pair in pairs.iter().take(5) {
            let verification = pair
                .verification
                .as_ref()
                .and_then(|v| v.similarity)
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "-".to_string());
            println!(
                "  {} -> {} | LSH {:.3} | verification {}",
                pair.source.name, pair.target.name, pair.similarity, verification
            );
        }
    }

    if !scored.is_empty() {
        let min = scored.iter().copied().fold(f64::INFINITY, f64::min);
        let max = scored.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean = scored.iter().sum::<f64>() / scored.len() as f64;
        println!(
            "Verification stats: mean {:.3} | min {:.3} | max {:.3}",
            mean, min, max
        );
    }

    Ok(())
}
