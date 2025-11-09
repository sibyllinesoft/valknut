//! Structure analysis detector - comprehensive directory refactor pack system.
//!
//! This module implements deterministic, LLM-free Directory Refactor Packs that compute
//! per-directory imbalance from file/subdir counts, LOC dispersion, and internal
//! dependencies; propose 2–4 subdirectory partitions via fast graph partitioning;
//! and emit File-Split Packs for whale files using intra-file cohesion analysis.
//!
//! Key features:
//! - Directory imbalance scoring using gini coefficient, entropy, and pressure metrics
//! - Graph-based directory partitioning with label propagation and Kernighan-Lin refinement
//! - Intra-file entity cohesion analysis for large file splitting recommendations
//! - Deterministic naming without AI/LLM dependencies
//! - Performance-optimized with SIMD and parallel processing
//! - Configurable thresholds and parameters via YAML

use std::collections::HashMap;
use std::path::Path;

use async_trait::async_trait;
use serde::Serialize;

use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};

pub mod config;
pub mod directory;
pub mod file;

pub use config::*;
use directory::DirectoryAnalyzer;
use file::FileAnalyzer;

/// Combined recommendation output containing both branch reorg and file split packs
#[derive(Debug, Serialize)]
pub struct StructureRecommendations {
    pub branch_reorg_packs: Vec<BranchReorgPack>,
    pub file_split_packs: Vec<FileSplitPack>,
}

impl StructureRecommendations {
    /// Get total number of recommendations
    pub fn len(&self) -> usize {
        self.branch_reorg_packs.len() + self.file_split_packs.len()
    }

    /// Check if there are no recommendations
    pub fn is_empty(&self) -> bool {
        self.branch_reorg_packs.is_empty() && self.file_split_packs.is_empty()
    }
}

impl IntoIterator for StructureRecommendations {
    type Item = serde_json::Value;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        let mut recommendations = Vec::new();

        // Add branch reorganization packs
        for pack in self.branch_reorg_packs {
            if let Ok(json) = serde_json::to_value(&pack) {
                recommendations.push(json);
            }
        }

        // Add file split packs
        for pack in self.file_split_packs {
            if let Ok(json) = serde_json::to_value(&pack) {
                recommendations.push(json);
            }
        }

        recommendations.into_iter()
    }
}

/// Main structure analysis extractor
pub struct StructureExtractor {
    config: StructureConfig,
    directory_analyzer: DirectoryAnalyzer,
    file_analyzer: FileAnalyzer,
    features: Vec<FeatureDefinition>,
}

impl Default for StructureExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl StructureExtractor {
    pub fn new() -> Self {
        let config = StructureConfig::default();
        Self::with_config(config)
    }

    pub fn with_config(config: StructureConfig) -> Self {
        let directory_analyzer = DirectoryAnalyzer::new(config.clone());
        let file_analyzer = FileAnalyzer::new(config.clone());

        let mut extractor = Self {
            config,
            directory_analyzer,
            file_analyzer,
            features: Vec::new(),
        };

        extractor.initialize_features();
        extractor
    }

    fn initialize_features(&mut self) {
        self.features = vec![
            FeatureDefinition::new(
                "directory_imbalance",
                "Overall imbalance score for directory structure",
            ),
            FeatureDefinition::new(
                "file_pressure",
                "File count pressure relative to configured maximum",
            ),
            FeatureDefinition::new(
                "branch_pressure",
                "Subdirectory count pressure relative to configured maximum",
            ),
            FeatureDefinition::new(
                "size_pressure",
                "Lines of code pressure relative to configured maximum",
            ),
            FeatureDefinition::new(
                "loc_dispersion",
                "Dispersion of lines of code across files (gini + entropy)",
            ),
            FeatureDefinition::new(
                "branch_reorg_value",
                "Value score for directory reorganization recommendation",
            ),
            FeatureDefinition::new(
                "file_split_value",
                "Value score for file splitting recommendation",
            ),
        ];
    }

    /// Generate comprehensive structure recommendations for a project
    pub async fn generate_recommendations(
        &self,
        root_path: &Path,
    ) -> Result<StructureRecommendations> {
        // Generate both types of packs in parallel
        let (branch_packs, file_packs) = tokio::join!(
            self.generate_branch_reorg_packs(root_path),
            self.generate_file_split_packs(root_path)
        );

        let mut branch_reorg_packs = branch_packs?;
        let mut file_split_packs = file_packs?;

        // Sort by impact/value and limit to configured top packs
        branch_reorg_packs.sort_by(|a, b| {
            b.gain
                .imbalance_delta
                .partial_cmp(&a.gain.imbalance_delta)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        branch_reorg_packs.truncate(self.config.top_packs);

        file_split_packs.sort_by(|a, b| {
            b.value
                .score
                .partial_cmp(&a.value.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        file_split_packs.truncate(self.config.top_packs);

        Ok(StructureRecommendations {
            branch_reorg_packs,
            file_split_packs,
        })
    }

    /// Generate branch reorganization packs
    async fn generate_branch_reorg_packs(&self, root_path: &Path) -> Result<Vec<BranchReorgPack>> {
        if !self.config.enable_branch_packs {
            return Ok(Vec::new());
        }

        let directories = self
            .directory_analyzer
            .discover_directories(root_path)
            .await?;

        let packs: Vec<BranchReorgPack> = directories
            .iter()
            .filter_map(|dir_path| {
                self.directory_analyzer
                    .analyze_directory_for_reorg(dir_path)
                    .ok()
                    .flatten()
            })
            .collect();

        Ok(packs)
    }

    /// Generate file split packs
    async fn generate_file_split_packs(&self, root_path: &Path) -> Result<Vec<FileSplitPack>> {
        if !self.config.enable_file_split_packs {
            return Ok(Vec::new());
        }

        let large_files = self.file_analyzer.discover_large_files(root_path).await?;

        let packs: Vec<FileSplitPack> = large_files
            .iter()
            .filter_map(|file_path| {
                self.file_analyzer
                    .analyze_file_for_split_with_root(file_path, root_path)
                    .ok()
                    .flatten()
            })
            .collect();

        Ok(packs)
    }

    /// Calculate directory metrics - exposed for testing and external use
    pub fn calculate_directory_metrics(&self, dir_path: &Path) -> Result<DirectoryMetrics> {
        self.directory_analyzer
            .calculate_directory_metrics(dir_path)
    }

    /// Analyze directory for reorganization - exposed for testing and external use
    pub fn analyze_directory_for_reorg(&self, dir_path: &Path) -> Result<Option<BranchReorgPack>> {
        self.directory_analyzer
            .analyze_directory_for_reorg(dir_path)
    }

    /// Analyze file for splitting - exposed for testing and external use
    pub fn analyze_file_for_split(&self, file_path: &Path) -> Result<Option<FileSplitPack>> {
        self.file_analyzer.analyze_file_for_split(file_path)
    }

    /// Calculate Gini coefficient - exposed for testing
    pub fn calculate_gini_coefficient(&self, values: &[usize]) -> f64 {
        self.directory_analyzer.calculate_gini_coefficient(values)
    }

    /// Calculate entropy - exposed for testing  
    pub fn calculate_entropy(&self, values: &[usize]) -> f64 {
        self.directory_analyzer.calculate_entropy(values)
    }

    /// Calculate size normalization factor - exposed for testing
    pub fn calculate_size_normalization_factor(&self, files: usize, total_loc: usize) -> f64 {
        self.directory_analyzer
            .calculate_size_normalization_factor(files, total_loc)
    }
}

#[async_trait]
impl FeatureExtractor for StructureExtractor {
    fn name(&self) -> &str {
        "structure"
    }

    fn features(&self) -> &[FeatureDefinition] {
        &self.features
    }

    async fn extract(
        &self,
        entity: &CodeEntity,
        _context: &ExtractionContext,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();

        // Extract directory-level features if entity represents a directory
        if let Some(dir_path) = std::path::Path::new(&entity.file_path).parent() {
            match self.calculate_directory_metrics(dir_path) {
                Ok(metrics) => {
                    features.insert("directory_imbalance".to_string(), metrics.imbalance);
                    features.insert("file_pressure".to_string(), metrics.file_pressure);
                    features.insert("branch_pressure".to_string(), metrics.branch_pressure);
                    features.insert("size_pressure".to_string(), metrics.size_pressure);
                    features.insert("loc_dispersion".to_string(), metrics.dispersion);

                    // Calculate branch reorg value
                    if let Ok(Some(_pack)) = self.analyze_directory_for_reorg(dir_path) {
                        features.insert("branch_reorg_value".to_string(), 0.8); // Would use actual value
                    } else {
                        features.insert("branch_reorg_value".to_string(), 0.0);
                    }
                }
                Err(_) => {
                    // Insert default values on error
                    features.insert("directory_imbalance".to_string(), 0.0);
                    features.insert("file_pressure".to_string(), 0.0);
                    features.insert("branch_pressure".to_string(), 0.0);
                    features.insert("size_pressure".to_string(), 0.0);
                    features.insert("loc_dispersion".to_string(), 0.0);
                    features.insert("branch_reorg_value".to_string(), 0.0);
                }
            }
        }

        // Extract file-level features
        if let Ok(Some(_pack)) =
            self.analyze_file_for_split(&std::path::Path::new(&entity.file_path))
        {
            features.insert("file_split_value".to_string(), 0.7); // Would use actual value
        } else {
            features.insert("file_split_value".to_string(), 0.0);
        }

        Ok(features)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::ValknutConfig;
    use crate::core::featureset::{CodeEntity, ExtractionContext};
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn sample_directory_metrics() -> DirectoryMetrics {
        DirectoryMetrics {
            files: 10,
            subdirs: 2,
            loc: 1_000,
            gini: 0.5,
            entropy: 0.8,
            file_pressure: 0.4,
            branch_pressure: 0.3,
            size_pressure: 0.6,
            dispersion: 0.55,
            imbalance: 0.9,
        }
    }

    fn sample_branch_pack() -> BranchReorgPack {
        BranchReorgPack {
            kind: "branch_reorg".to_string(),
            dir: PathBuf::from("src"),
            current: sample_directory_metrics(),
            proposal: vec![DirectoryPartition {
                name: "src/core".to_string(),
                files: vec![PathBuf::from("src/core/lib.rs")],
                loc: 600,
            }],
            file_moves: vec![FileMove {
                from: PathBuf::from("src/lib.rs"),
                to: PathBuf::from("src/core/lib.rs"),
            }],
            gain: ReorganizationGain {
                imbalance_delta: 0.3,
                cross_edges_reduced: 2,
            },
            effort: ReorganizationEffort {
                files_moved: 1,
                import_updates_est: 0,
            },
            rules: vec!["Preserve module boundaries".to_string()],
        }
    }

    fn sample_file_split_pack() -> FileSplitPack {
        FileSplitPack {
            kind: "file_split".to_string(),
            file: PathBuf::from("src/big.rs"),
            reasons: vec!["loc 2500 > 1500".to_string()],
            suggested_splits: vec![SuggestedSplit {
                name: "src/big_extract.rs".to_string(),
                entities: vec!["process".to_string(), "handle".to_string()],
                loc: 800,
            }],
            value: SplitValue { score: 0.75 },
            effort: SplitEffort {
                exports: 2,
                external_importers: 1,
            },
        }
    }

    #[test]
    fn structure_recommendations_iterates_all_packs() {
        let recommendations = StructureRecommendations {
            branch_reorg_packs: vec![sample_branch_pack()],
            file_split_packs: vec![sample_file_split_pack()],
        };

        assert_eq!(recommendations.len(), 2);
        assert!(!recommendations.is_empty());

        let json_values: Vec<_> = recommendations.into_iter().collect();
        assert_eq!(json_values.len(), 2);
        assert!(json_values
            .iter()
            .any(|value| value.get("kind") == Some(&Value::String("branch_reorg".into()))));
    }

    #[tokio::test]
    async fn structure_extractor_respects_disabled_packs() {
        let temp = tempdir().expect("temp dir");
        let mut config = StructureConfig::default();
        config.enable_branch_packs = false;
        config.enable_file_split_packs = false;
        let extractor = StructureExtractor::with_config(config);

        let recommendations = extractor
            .generate_recommendations(temp.path())
            .await
            .expect("generate recommendations");

        assert!(recommendations.is_empty());
    }

    #[test]
    fn structure_extractor_registers_expected_features() {
        let extractor = StructureExtractor::new();
        assert_eq!(extractor.name(), "structure");

        let feature_names: Vec<_> = extractor
            .features()
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(
            feature_names,
            vec![
                "directory_imbalance",
                "file_pressure",
                "branch_pressure",
                "size_pressure",
                "loc_dispersion",
                "branch_reorg_value",
                "file_split_value"
            ]
        );
    }

    #[tokio::test]
    async fn structure_extractor_extract_returns_defaults_on_error() {
        let extractor = StructureExtractor::default();
        let config = Arc::new(ValknutConfig::default());
        let context = ExtractionContext::new(config, "rust");

        let entity = CodeEntity::new("entity-1", "File", "missing.rs", "/tmp/missing.rs")
            .with_line_range(1, 10);

        let features = extractor
            .extract(&entity, &context)
            .await
            .expect("extract features");

        assert_eq!(extractor.features().len(), 7);
        for key in [
            "directory_imbalance",
            "file_pressure",
            "branch_pressure",
            "size_pressure",
            "loc_dispersion",
            "branch_reorg_value",
            "file_split_value",
        ] {
            let value = *features
                .get(key)
                .unwrap_or_else(|| panic!("missing feature {key}"));
            assert!(
                value >= 0.0 && value <= 1.0,
                "expected normalized value for feature {key}, got {value}"
            );
        }

        assert_eq!(features["file_split_value"], 0.0);
    }

    #[tokio::test]
    async fn structure_extractor_extracts_and_caches_directory_metrics() {
        let temp = tempdir().expect("temp dir");
        let src_dir = temp.path().join("src");
        std::fs::create_dir(&src_dir).expect("create src directory");

        let file_path = src_dir.join("lib.rs");
        std::fs::write(&file_path, "fn demo() {}\n// comment\nfn helper() {}\n")
            .expect("write file");

        let extractor = StructureExtractor::new();

        let first_metrics = extractor
            .calculate_directory_metrics(&src_dir)
            .expect("metrics");
        assert_eq!(first_metrics.files, 1);

        let cached_metrics = extractor
            .calculate_directory_metrics(&src_dir)
            .expect("cached metrics");
        assert!(
            (first_metrics.imbalance - cached_metrics.imbalance).abs() < f64::EPSILON,
            "cached metrics should match initial computation"
        );

        let entity = CodeEntity::new(
            "entity-src",
            "module",
            "demo",
            file_path.to_string_lossy().to_string(),
        )
        .with_line_range(1, 4);
        let context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "rust");
        let features = extractor
            .extract(&entity, &context)
            .await
            .expect("extract features");

        assert!(features.contains_key("directory_imbalance"));
        assert!(features.contains_key("file_split_value"));
    }

    #[test]
    fn structure_extractor_exposes_statistical_helpers() {
        let extractor = StructureExtractor::default();
        assert!((extractor.calculate_gini_coefficient(&[1, 1, 1]) - 0.0).abs() < 1e-6);
        assert!((extractor.calculate_entropy(&[1, 1]) - 1.0).abs() < 1e-6);
        assert!(
            extractor.calculate_size_normalization_factor(10, 1_000) >= 1.0,
            "size normalization should be non-zero"
        );
    }
}
