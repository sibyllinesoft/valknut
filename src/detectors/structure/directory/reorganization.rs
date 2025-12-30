//! Reorganization planning and gain calculation for directory analysis.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::errors::{Result, ValknutError};
use crate::detectors::structure::config::{
    DependencyGraph, DirectoryMetrics, DirectoryPartition, FileMove, ReorganizationEffort,
    ReorganizationGain, StructureConfig,
};

use super::stats::{
    calculate_entropy, calculate_gini_coefficient, calculate_size_normalization_factor,
};

/// Reorganization planner for directory restructuring.
pub struct ReorganizationPlanner<'a> {
    config: &'a StructureConfig,
}

impl<'a> ReorganizationPlanner<'a> {
    pub fn new(config: &'a StructureConfig) -> Self {
        Self { config }
    }

    /// Calculate expected gains from reorganization
    pub fn calculate_reorganization_gain(
        &self,
        current_metrics: &DirectoryMetrics,
        partitions: &[DirectoryPartition],
        dir_path: &Path,
        build_dependency_graph: impl Fn(&Path) -> Result<DependencyGraph>,
    ) -> Result<ReorganizationGain> {
        // Calculate imbalance for each proposed partition
        let mut partition_imbalances = Vec::new();

        for partition in partitions {
            // Create a temporary directory metrics for this partition
            let partition_files = partition.files.len();
            let _partition_subdirs = 0; // New partitions start with 0 subdirs
            let partition_loc = partition.loc;

            // Simulate LOC distribution within partition (simplified)
            let avg_loc_per_file = if partition_files > 0 {
                partition_loc / partition_files
            } else {
                0
            };
            let loc_distribution: Vec<usize> =
                (0..partition_files).map(|_| avg_loc_per_file).collect();

            // Calculate metrics for this partition
            let gini = calculate_gini_coefficient(&loc_distribution);
            let entropy = calculate_entropy(&loc_distribution);

            // Calculate pressure metrics
            let file_pressure =
                (partition_files as f64 / self.config.fsdir.max_files_per_dir as f64).min(1.0);
            let branch_pressure = 0.0; // No subdirs in new partition
            let size_pressure =
                (partition_loc as f64 / self.config.fsdir.max_dir_loc as f64).min(1.0);

            // Calculate dispersion
            let max_entropy = if partition_files > 0 {
                (partition_files as f64).log2()
            } else {
                1.0
            };
            let normalized_entropy = if max_entropy > 0.0 {
                entropy / max_entropy
            } else {
                0.0
            };
            let dispersion = gini.max(1.0 - normalized_entropy);

            // Apply size normalization
            let size_normalization_factor =
                calculate_size_normalization_factor(partition_files, partition_loc);

            // Calculate imbalance for this partition
            let raw_imbalance = 0.35 * file_pressure
                + 0.25 * branch_pressure
                + 0.25 * size_pressure
                + 0.15 * dispersion;

            let partition_imbalance = raw_imbalance * size_normalization_factor;
            partition_imbalances.push(partition_imbalance);
        }

        // Calculate average imbalance of new partitions
        let avg_new_imbalance = if !partition_imbalances.is_empty() {
            partition_imbalances.iter().sum::<f64>() / partition_imbalances.len() as f64
        } else {
            current_metrics.imbalance
        };

        // Imbalance improvement (positive means improvement)
        let imbalance_delta = (current_metrics.imbalance - avg_new_imbalance).max(0.0);

        // Calculate cross-edges reduced by analyzing dependency graph
        let cross_edges_reduced =
            self.estimate_cross_edges_reduced(partitions, dir_path, build_dependency_graph)?;

        Ok(ReorganizationGain {
            imbalance_delta,
            cross_edges_reduced,
        })
    }

    /// Estimate how many cross-partition edges would be reduced
    fn estimate_cross_edges_reduced(
        &self,
        partitions: &[DirectoryPartition],
        dir_path: &Path,
        build_dependency_graph: impl Fn(&Path) -> Result<DependencyGraph>,
    ) -> Result<usize> {
        // Build dependency graph to analyze edge cuts
        let dependency_graph = build_dependency_graph(dir_path)?;

        // Create partition mapping
        let mut file_to_partition: HashMap<PathBuf, usize> = HashMap::new();
        for (partition_idx, partition) in partitions.iter().enumerate() {
            for file_path in &partition.files {
                file_to_partition.insert(file_path.clone(), partition_idx);
            }
        }

        // Count edges that would cross partition boundaries
        let mut cross_edges = 0;
        let mut _total_internal_edges = 0;

        for edge_idx in dependency_graph.edge_indices() {
            if let Some((source, target)) = dependency_graph.edge_endpoints(edge_idx) {
                if let (Some(source_node), Some(target_node)) = (
                    dependency_graph.node_weight(source),
                    dependency_graph.node_weight(target),
                ) {
                    _total_internal_edges += 1;

                    // Check if this edge would cross partition boundaries
                    if let (Some(&source_partition), Some(&target_partition)) = (
                        file_to_partition.get(&source_node.path),
                        file_to_partition.get(&target_node.path),
                    ) {
                        if source_partition != target_partition {
                            cross_edges += 1;
                        }
                    }
                }
            }
        }

        // Return estimated edges that would be internal after reorganization
        Ok(cross_edges)
    }

    /// Calculate effort estimation for reorganization
    pub fn calculate_reorganization_effort(
        &self,
        partitions: &[DirectoryPartition],
    ) -> Result<ReorganizationEffort> {
        let files_moved = partitions.iter().map(|p| p.files.len()).sum();

        // Rough estimation: 2 import updates per moved file on average
        let import_updates_est = files_moved * 2;

        Ok(ReorganizationEffort {
            files_moved,
            import_updates_est,
        })
    }

    /// Generate file moves for reorganization
    pub fn generate_file_moves(
        &self,
        partitions: &[DirectoryPartition],
        dir_path: &Path,
    ) -> Result<Vec<FileMove>> {
        let mut file_moves = Vec::new();

        for partition in partitions {
            for file_path in &partition.files {
                // Create destination path in new subdirectory
                let file_name = file_path
                    .file_name()
                    .ok_or_else(|| ValknutError::internal("Invalid file path"))?;

                let destination = dir_path.join(&partition.name).join(file_name);

                file_moves.push(FileMove {
                    from: file_path.clone(),
                    to: destination,
                });
            }
        }

        Ok(file_moves)
    }

    /// Generate reorganization rules
    pub fn generate_reorganization_rules(&self) -> Vec<String> {
        vec![
            "Create subdirectories for each partition".to_string(),
            "Update relative import statements".to_string(),
            "Preserve file names and structure within partitions".to_string(),
            "Test imports after reorganization".to_string(),
        ]
    }
}
