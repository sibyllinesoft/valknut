//! Scoring conversion utilities for the analysis pipeline.
//!
//! This module provides functions for converting comprehensive analysis results
//! into scoring results, feature vectors, and health metrics.

use std::collections::{HashMap, HashSet};

use crate::core::featureset::FeatureVector;
use crate::core::scoring::{Priority, ScoringResult};
use crate::detectors::complexity::ComplexitySeverity;

use crate::core::pipeline::results::pipeline_results::{ComprehensiveAnalysisResult, HealthMetrics};

/// Compute health metrics from scoring results.
pub fn health_from_scores(scoring: &[ScoringResult]) -> HealthMetrics {
    if scoring.is_empty() {
        return HealthMetrics {
            overall_health_score: 100.0,
            maintainability_score: 100.0,
            technical_debt_ratio: 0.0,
            complexity_score: 0.0,
            structure_quality_score: 100.0,
            doc_health_score: 100.0,
        };
    }

    let avg_abs_score = scoring
        .iter()
        .map(|result| result.overall_score.abs())
        .sum::<f64>()
        / scoring.len() as f64;

    let overall_health = (100.0 - avg_abs_score * 20.0).clamp(0.0, 100.0);
    let maintainability = (100.0 - avg_abs_score * 18.0).clamp(0.0, 100.0);
    let technical_debt = (avg_abs_score * 25.0).clamp(0.0, 100.0);
    let complexity = (avg_abs_score * 30.0).clamp(0.0, 100.0);
    let structure_quality = (100.0 - avg_abs_score * 12.0).clamp(0.0, 100.0);
    let doc_health_score = 100.0; // placeholder until doc analysis contributes

    HealthMetrics {
        overall_health_score: overall_health,
        maintainability_score: maintainability,
        technical_debt_ratio: technical_debt,
        complexity_score: complexity,
        structure_quality_score: structure_quality,
        doc_health_score,
    }
}

/// Convert comprehensive analysis results to scoring results.
pub fn convert_to_scoring_results(results: &ComprehensiveAnalysisResult) -> Vec<ScoringResult> {
    let mut scoring_results = Vec::new();

    // Helper closure to clamp values into scoring range
    let clamp_score = |value: f64| value.clamp(0.0, 100.0);

    // Convert complexity analysis results to scoring results
    for complexity_result in &results.complexity.detailed_results {
        let entity_id = format!(
            "{}:{}:{}",
            complexity_result.file_path,
            complexity_result.entity_type,
            complexity_result.entity_name
        );

        let metrics = &complexity_result.metrics;

        // Normalise metrics against reasonable thresholds
        let cyclomatic_score = clamp_score((metrics.cyclomatic() / 10.0) * 40.0);
        let cognitive_score = clamp_score((metrics.cognitive() / 15.0) * 30.0);
        let nesting_score = clamp_score(metrics.max_nesting_depth * 6.0);
        let debt_score = clamp_score(metrics.technical_debt_score);
        let maintainability_penalty = clamp_score(100.0 - metrics.maintainability_index);

        let mut category_scores = HashMap::new();
        category_scores.insert("complexity".to_string(), cyclomatic_score);
        category_scores.insert("cognitive".to_string(), cognitive_score);
        category_scores.insert("structure".to_string(), nesting_score);
        category_scores.insert("debt".to_string(), debt_score);
        category_scores.insert("maintainability".to_string(), maintainability_penalty);

        let mut feature_contributions = HashMap::new();
        feature_contributions.insert("cyclomatic_complexity".to_string(), metrics.cyclomatic());
        feature_contributions.insert("cognitive_complexity".to_string(), metrics.cognitive());
        feature_contributions.insert("max_nesting_depth".to_string(), metrics.max_nesting_depth);
        feature_contributions.insert("lines_of_code".to_string(), metrics.lines_of_code);
        feature_contributions.insert(
            "technical_debt_score".to_string(),
            metrics.technical_debt_score,
        );
        feature_contributions.insert(
            "maintainability_index".to_string(),
            metrics.maintainability_index,
        );

        let weighted_overall = clamp_score(
            cyclomatic_score * 0.30
                + cognitive_score * 0.25
                + nesting_score * 0.15
                + debt_score * 0.20
                + maintainability_penalty * 0.10,
        );

        let mut priority = match complexity_result.severity {
            ComplexitySeverity::Critical => Priority::Critical,
            ComplexitySeverity::VeryHigh => Priority::High,
            ComplexitySeverity::High => Priority::High,
            ComplexitySeverity::Medium => Priority::Medium,
            ComplexitySeverity::Moderate => Priority::Medium,
            ComplexitySeverity::Low => Priority::Low,
        };

        if complexity_result.issues.is_empty() {
            priority = if weighted_overall >= 70.0 {
                Priority::Critical
            } else if weighted_overall >= 55.0 {
                Priority::High
            } else if weighted_overall >= 35.0 {
                Priority::Medium
            } else if weighted_overall >= 20.0 {
                Priority::Low
            } else {
                Priority::None
            };
        }

        let confidence = if metrics.lines_of_code >= 30.0 {
            0.95
        } else if metrics.lines_of_code >= 15.0 {
            0.85
        } else if metrics.lines_of_code >= 5.0 {
            0.7
        } else {
            0.5
        };

        let feature_count = feature_contributions.len();
        scoring_results.push(ScoringResult {
            entity_id,
            overall_score: weighted_overall,
            priority,
            category_scores,
            feature_contributions,
            normalized_feature_count: feature_count,
            confidence,
        });
    }

    // Convert refactoring analysis results to scoring results
    for refactoring_result in &results.refactoring.detailed_results {
        let entity_id = format!(
            "{}:refactoring:{}",
            refactoring_result.file_path,
            refactoring_result.recommendations.len()
        );

        // Map refactoring metrics to scoring categories
        let mut category_scores = HashMap::new();
        let refactoring_score = refactoring_result.refactoring_score;
        category_scores.insert("refactoring".to_string(), refactoring_score);

        // Map individual features to contributions
        let mut feature_contributions = HashMap::new();
        feature_contributions.insert("refactoring_score".to_string(), refactoring_score);
        feature_contributions.insert(
            "refactoring_recommendations".to_string(),
            refactoring_result.recommendations.len() as f64,
        );

        // Calculate overall score based on refactoring needs
        let overall_score = clamp_score(refactoring_score);

        let priority = if overall_score >= 75.0 {
            Priority::Critical
        } else if overall_score >= 55.0 {
            Priority::High
        } else if overall_score >= 35.0 {
            Priority::Medium
        } else if overall_score >= 20.0 {
            Priority::Low
        } else {
            Priority::None
        };

        // High confidence for refactoring analysis
        let confidence = 0.85;

        if priority != Priority::None {
            let feature_count = feature_contributions.len();
            scoring_results.push(ScoringResult {
                entity_id,
                overall_score,
                priority,
                category_scores,
                feature_contributions,
                normalized_feature_count: feature_count,
                confidence,
            });
        }
    }

    scoring_results
}

/// Create feature vectors from comprehensive analysis results.
pub fn create_feature_vectors_from_results(results: &ComprehensiveAnalysisResult) -> Vec<FeatureVector> {
    let mut feature_vectors = Vec::new();

    // Collect per-file aggregates for structure metrics
    let mut files: HashSet<String> = HashSet::new();
    let mut func_counts: HashMap<String, usize> = HashMap::new();
    let mut class_counts: HashMap<String, usize> = HashMap::new();

    for c in &results.complexity.detailed_results {
        files.insert(c.file_path.clone());
        match c.entity_type.as_str() {
            "function" => *func_counts.entry(c.file_path.clone()).or_insert(0) += 1,
            "class" => *class_counts.entry(c.file_path.clone()).or_insert(0) += 1,
            _ => {}
        }
    }
    for r in &results.refactoring.detailed_results {
        files.insert(r.file_path.clone());
    }

    // LOC per file from disk (best-effort)
    let mut file_loc: HashMap<String, f64> = HashMap::new();
    for file in &files {
        let loc = std::fs::read_to_string(file)
            .map(|c| c.lines().count() as f64)
            .unwrap_or(0.0);
        file_loc.insert(file.clone(), loc);
    }

    // Files per directory
    let mut files_per_dir: HashMap<String, usize> = HashMap::new();
    for file in &files {
        let dir = std::path::Path::new(file)
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());
        *files_per_dir.entry(dir).or_insert(0) += 1;
    }

    // Create per-file feature vectors with normalized structure metrics
    for file in &files {
        let loc = *file_loc.get(file).unwrap_or(&0.0);
        let funcs = *func_counts.get(file).unwrap_or(&0) as f64;
        let classes = *class_counts.get(file).unwrap_or(&0) as f64;
        let dir = std::path::Path::new(file)
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());
        let dir_files = *files_per_dir.get(&dir).unwrap_or(&1) as f64;

        let mut feature_vector = FeatureVector::new(format!("{}:file", file));
        feature_vector.add_feature("lines_of_code", loc);
        feature_vector.add_feature("functions_per_file", funcs);
        feature_vector.add_feature("classes_per_file", classes);
        feature_vector.add_feature("files_per_directory", dir_files);

        // Normalized severities (higher is worse)
        feature_vector
            .normalized_features
            .insert("lines_of_code".to_string(), logistic_over(loc, 300.0, 75.0));
        feature_vector.normalized_features.insert(
            "functions_per_file".to_string(),
            logistic_over(funcs, 12.0, 4.0),
        );
        feature_vector.normalized_features.insert(
            "classes_per_file".to_string(),
            logistic_over(classes, 2.0, 1.0),
        );
        feature_vector.normalized_features.insert(
            "files_per_directory".to_string(),
            logistic_over(dir_files, 7.0, 2.0),
        );

        feature_vector
            .add_metadata("entity_type", serde_json::Value::String("file".to_string()));
        feature_vector.add_metadata("file_path", serde_json::Value::String(file.clone()));

        feature_vectors.push(feature_vector);
    }

    // Create feature vectors from complexity analysis results
    for complexity_result in &results.complexity.detailed_results {
        let entity_id = format!(
            "{}:{}:{}",
            complexity_result.file_path,
            complexity_result.entity_type,
            complexity_result.entity_name
        );

        let metrics = &complexity_result.metrics;

        // Create feature vector with features and their values
        let mut feature_vector = FeatureVector::new(entity_id.clone());

        // Add raw feature values
        feature_vector.add_feature("cyclomatic_complexity", metrics.cyclomatic());
        feature_vector.add_feature("cognitive_complexity", metrics.cognitive());
        feature_vector.add_feature("max_nesting_depth", metrics.max_nesting_depth);
        feature_vector.add_feature("lines_of_code", metrics.lines_of_code);
        feature_vector.add_feature("technical_debt_score", metrics.technical_debt_score);
        feature_vector.add_feature("maintainability_index", metrics.maintainability_index);

        // Add normalized versions (simple normalization for now)
        feature_vector.normalized_features.insert(
            "cyclomatic_complexity".to_string(),
            (metrics.cyclomatic() / 10.0).min(1.0),
        );
        feature_vector.normalized_features.insert(
            "cognitive_complexity".to_string(),
            (metrics.cognitive() / 15.0).min(1.0),
        );
        feature_vector.normalized_features.insert(
            "max_nesting_depth".to_string(),
            (metrics.max_nesting_depth / 5.0).min(1.0),
        );
        feature_vector.normalized_features.insert(
            "lines_of_code".to_string(),
            (metrics.lines_of_code / 100.0).min(1.0),
        );
        feature_vector.normalized_features.insert(
            "technical_debt_score".to_string(),
            metrics.technical_debt_score / 100.0,
        );
        feature_vector.normalized_features.insert(
            "maintainability_index".to_string(),
            metrics.maintainability_index / 100.0,
        );

        // Set metadata
        feature_vector.add_metadata(
            "entity_type",
            serde_json::Value::String(complexity_result.entity_type.clone()),
        );
        feature_vector.add_metadata(
            "file_path",
            serde_json::Value::String(complexity_result.file_path.clone()),
        );
        feature_vector.add_metadata("language", serde_json::Value::String("Python".to_string()));
        feature_vector.add_metadata(
            "line_number",
            serde_json::Value::Number(complexity_result.start_line.into()),
        );

        feature_vectors.push(feature_vector);
    }

    // Create feature vectors from refactoring analysis results
    for refactoring_result in &results.refactoring.detailed_results {
        let entity_id = format!(
            "{}:refactoring:{}",
            refactoring_result.file_path,
            refactoring_result.recommendations.len()
        );

        let mut feature_vector = FeatureVector::new(entity_id.clone());

        // Add refactoring-specific features
        feature_vector.add_feature("refactoring_score", refactoring_result.refactoring_score);
        feature_vector.add_feature(
            "refactoring_recommendations",
            refactoring_result.recommendations.len() as f64,
        );

        // Add normalized versions
        feature_vector.normalized_features.insert(
            "refactoring_score".to_string(),
            refactoring_result.refactoring_score / 100.0,
        );
        feature_vector.normalized_features.insert(
            "refactoring_recommendations".to_string(),
            (refactoring_result.recommendations.len() as f64 / 10.0).min(1.0),
        );

        // Set metadata
        feature_vector.add_metadata(
            "entity_type",
            serde_json::Value::String("refactoring".to_string()),
        );
        feature_vector.add_metadata(
            "file_path",
            serde_json::Value::String(refactoring_result.file_path.clone()),
        );
        feature_vector.add_metadata("language", serde_json::Value::String("Python".to_string()));

        feature_vectors.push(feature_vector);
    }

    feature_vectors
}

/// Logistic mapping that trends to 1.0 as value grows past mid.
fn logistic_over(value: f64, mid: f64, steepness: f64) -> f64 {
    let k = if steepness <= 0.0 { 1.0 } else { steepness };
    let exponent = -((value - mid) / k);
    let denom = 1.0 + exponent.exp();
    (1.0 / denom).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_from_scores_empty() {
        let metrics = health_from_scores(&[]);
        assert_eq!(metrics.overall_health_score, 100.0);
        assert_eq!(metrics.technical_debt_ratio, 0.0);
    }

    #[test]
    fn test_logistic_over() {
        // Value below mid should be < 0.5
        assert!(logistic_over(100.0, 300.0, 75.0) < 0.5);
        // Value at mid should be ~0.5
        let at_mid = logistic_over(300.0, 300.0, 75.0);
        assert!((at_mid - 0.5).abs() < 0.01);
        // Value above mid should be > 0.5
        assert!(logistic_over(500.0, 300.0, 75.0) > 0.5);
    }
}
