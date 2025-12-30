//! LSH analysis stage for clone detection in the pipeline.
//!
//! This module handles LSH-based clone detection, entity collection,
//! and APTED verification for code similarity analysis.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tracing::{debug, info, warn};

use super::clone_detection::{
    compute_apted_limit, compute_apted_verification, filter_small_pairs, log_partition_stats,
    ordered_pair_key, serialize_clone_pairs, should_skip_small_pair, CachedSimpleAst,
    CloneDetectionStats, CloneEndpoint, ClonePairReport, LshDetectionParams, LshEntityCollection,
};
use super::pipeline_results::LshAnalysisResults;
use crate::core::ast_service::{AstService, CachedTree};
use crate::core::config::ValknutConfig;
use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext};
use crate::detectors::graph::SimilarityCliquePartitioner;
use crate::detectors::lsh::{LshExtractor, LshSimilarityContext};

/// LSH analysis stage implementation.
pub struct LshStage<'a> {
    lsh_extractor: &'a LshExtractor,
    ast_service: Arc<AstService>,
    valknut_config: Arc<ValknutConfig>,
}

impl<'a> LshStage<'a> {
    /// Create a new LSH stage with the given components.
    pub fn new(
        lsh_extractor: &'a LshExtractor,
        ast_service: Arc<AstService>,
        valknut_config: Arc<ValknutConfig>,
    ) -> Self {
        Self {
            lsh_extractor,
            ast_service,
            valknut_config,
        }
    }

    /// Run LSH analysis for clone detection.
    pub async fn run_lsh_analysis(
        &self,
        files: &[PathBuf],
        denoise_enabled: bool,
    ) -> Result<LshAnalysisResults> {
        const MAX_ENTITIES_PER_FILE_FOR_LSH: usize = 1500;
        debug!(
            "Running LSH analysis for clone detection on {} files",
            files.len()
        );

        let lsh_settings = &self.valknut_config.lsh;
        let verify_with_apted = lsh_settings.verify_with_apted;
        let apted_max_nodes = lsh_settings.apted_max_nodes;
        let apted_limit = compute_apted_limit(lsh_settings);

        let collection = self
            .collect_entities_for_lsh(
                files,
                verify_with_apted,
                MAX_ENTITIES_PER_FILE_FOR_LSH,
            )
            .await;

        let LshEntityCollection {
            entities,
            entity_index,
            ast_cache,
        } = collection;

        if entities.is_empty() {
            info!("No entities available for LSH after filtering; skipping clone analysis");
            return Ok(LshAnalysisResults::empty_with_settings(
                verify_with_apted,
                denoise_enabled,
            ));
        }

        let mut context = ExtractionContext::new(Arc::clone(&self.valknut_config), "mixed");
        context.entity_index = entity_index;

        let partitions = SimilarityCliquePartitioner::new().partition(&entities);
        if !partitions.is_empty() {
            log_partition_stats(&partitions);
            context.candidate_partitions = Some(Arc::new(partitions));
        }

        let similarity_context = self.lsh_extractor.similarity_context(&context);
        if similarity_context.is_none() {
            warn!("Unable to build LSH similarity context; clone pairs will not be generated");
        }

        let candidate_limit = self.lsh_extractor.max_candidates();
        let min_ast_nodes = self.lsh_extractor.min_ast_nodes_threshold().unwrap_or(0);
        info!(min_ast_nodes, "LSH clone pair filter min_ast_nodes threshold");

        let lsh_threshold = self.lsh_extractor.similarity_threshold();

        let (clone_pairs, stats) = self
            .detect_clone_pairs(
                &entities,
                &context,
                similarity_context.as_deref(),
                &ast_cache,
                LshDetectionParams {
                    candidate_limit,
                    min_ast_nodes,
                    lsh_threshold,
                    verify_with_apted,
                    apted_limit,
                    apted_max_nodes,
                },
            )
            .await;

        let clone_pairs = filter_small_pairs(clone_pairs, min_ast_nodes);
        let clone_pair_count = clone_pairs.len();
        let serialized_pairs = serialize_clone_pairs(clone_pairs, min_ast_nodes);

        Ok(LshAnalysisResults {
            enabled: true,
            clone_pairs: serialized_pairs,
            max_similarity: stats.max_similarity,
            avg_similarity: stats.avg_similarity(),
            duplicate_count: clone_pair_count,
            apted_verification_enabled: verify_with_apted,
            verification: stats.verification_summary(verify_with_apted),
            denoising_enabled: denoise_enabled,
            tfidf_stats: if denoise_enabled {
                Some(super::pipeline_results::TfIdfStats::default())
            } else {
                None
            },
        })
    }

    /// Detect clone pairs from entities.
    async fn detect_clone_pairs(
        &self,
        entities: &[CodeEntity],
        context: &ExtractionContext,
        similarity_context: Option<&LshSimilarityContext>,
        ast_cache: &HashMap<String, Arc<CachedTree>>,
        params: LshDetectionParams,
    ) -> (Vec<ClonePairReport>, CloneDetectionStats) {
        let mut clone_pairs = Vec::new();
        let mut seen_pairs: HashSet<(String, String)> = HashSet::new();
        let mut stats = CloneDetectionStats::default();
        let mut simple_ast_cache: HashMap<String, Option<CachedSimpleAst>> = HashMap::new();

        let Some(ctx) = similarity_context else {
            return (clone_pairs, stats);
        };

        for entity in entities {
            let candidates = ctx.find_similar_entities(&entity.id, params.candidate_limit);
            let mut apted_evaluated = 0usize;

            for (candidate_id, similarity) in candidates {
                if similarity < params.lsh_threshold {
                    continue;
                }

                let key = ordered_pair_key(&entity.id, &candidate_id);
                if !seen_pairs.insert(key) {
                    continue;
                }

                let Some(candidate_entity) = context.entity_index.get(&candidate_id) else {
                    continue;
                };

                stats.record_similarity(similarity);

                let apted_allowed = params.verify_with_apted
                    && params.apted_limit.map_or(true, |limit| apted_evaluated < limit);

                let verification_detail = if apted_allowed {
                    stats.apted_pairs_requested += 1;
                    let detail = compute_apted_verification(
                        entity,
                        candidate_entity,
                        &mut simple_ast_cache,
                        ast_cache,
                        params.apted_max_nodes,
                    )
                    .await;
                    stats.record_verification(&detail, &mut apted_evaluated);
                    detail
                } else {
                    None
                };

                if should_skip_small_pair(&verification_detail, params.min_ast_nodes, entity, candidate_entity) {
                    continue;
                }

                clone_pairs.push(ClonePairReport {
                    source: CloneEndpoint::from_entity(entity),
                    target: CloneEndpoint::from_entity(candidate_entity),
                    similarity,
                    verification: verification_detail,
                });
            }
        }

        (clone_pairs, stats)
    }

    /// Collect entities from files for LSH clone detection analysis.
    async fn collect_entities_for_lsh(
        &self,
        files: &[PathBuf],
        verify_with_apted: bool,
        max_entities_per_file: usize,
    ) -> LshEntityCollection {
        let mut collection = LshEntityCollection::new();

        for file_path in files.iter() {
            let content = match tokio::fs::read_to_string(file_path).await {
                Ok(content) => content,
                Err(e) => {
                    warn!("Failed to read file {}: {}", file_path.display(), e);
                    continue;
                }
            };

            let path_str = file_path.to_string_lossy().to_string();

            // Build AST cache if APTED verification is enabled
            if verify_with_apted {
                match self.ast_service.get_ast(&path_str, &content).await {
                    Ok(tree) => {
                        collection.ast_cache.insert(path_str.clone(), tree);
                    }
                    Err(e) => {
                        warn!(
                            "Failed to parse AST for {}: {} – APTED verification will be skipped for entities in this file",
                            file_path.display(),
                            e
                        );
                    }
                }
            }

            // Extract entities from the file
            let Some(extracted_entities) = self.extract_entities_from_file(file_path, &content).await else {
                continue;
            };

            if extracted_entities.len() > max_entities_per_file {
                info!(
                    file = %file_path.display(),
                    entities = extracted_entities.len(),
                    "Skipping LSH for file with excessive entity count"
                );
                continue;
            }

            // Filter entities through LSH thresholds
            for entity in extracted_entities {
                if !self.lsh_extractor
                    .entity_passes_thresholds(&entity)
                    .await
                    .unwrap_or(false)
                {
                    continue;
                }

                collection.entity_index.insert(entity.id.clone(), entity.clone());
                collection.entities.push(entity);
            }
        }

        collection
    }

    /// Extract entities from a file using appropriate language adapter.
    async fn extract_entities_from_file(
        &self,
        file_path: &Path,
        content: &str,
    ) -> Option<Vec<CodeEntity>> {
        use crate::lang::registry::adapter_for_file;

        // Get appropriate language adapter
        let mut adapter = match adapter_for_file(file_path) {
            Ok(adapter) => adapter,
            Err(e) => {
                debug!("No language adapter for {}: {}", file_path.display(), e);
                return None;
            }
        };

        // Extract entities using the standardized interface
        match adapter.extract_code_entities(content, &file_path.to_string_lossy()) {
            Ok(entities) => {
                debug!(
                    "Extracted {} entities from {}",
                    entities.len(),
                    file_path.display()
                );
                Some(entities)
            }
            Err(e) => {
                warn!(
                    "Failed to extract entities from {}: {}",
                    file_path.display(),
                    e
                );
                None
            }
        }
    }
}
