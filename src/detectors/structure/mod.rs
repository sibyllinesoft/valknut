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
