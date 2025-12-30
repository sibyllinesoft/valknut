//! Semantic cohesion analysis detector.
//!
//! This module implements semantic cohesion analysis using embeddings to detect:
//! - Code units that "do multiple things" (low cohesion)
//! - Documentation that doesn't match code semantics (doc mismatch)
//! - Semantic outliers (functions/files that don't belong)
//!
//! Key features:
//! - Symbol-only text embeddings (avoids full code for speed)
//! - Robust centroid roll-ups from function → file → folder
//! - Cohesion via vector concentration (mean cosine to centroid)
//! - Doc↔code alignment via centroid similarity
//! - Configurable thresholds with percentile-based defaults

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::core::errors::Result;
use crate::core::featureset::{CodeEntity, ExtractionContext, FeatureDefinition, FeatureExtractor};

pub mod config;
pub mod embeddings;
pub mod extractor;
pub mod metrics;
pub mod symbols;

pub use config::*;
pub use extractor::CohesionEntity;
use embeddings::EmbeddingProvider;
use metrics::CohesionCalculator;

/// Results from cohesion analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohesionAnalysisResults {
    /// Whether cohesion analysis was enabled
    pub enabled: bool,
    /// Per-file cohesion scores
    pub file_scores: HashMap<PathBuf, FileCohesionScore>,
    /// Per-folder cohesion scores
    pub folder_scores: HashMap<PathBuf, FolderCohesionScore>,
    /// Detected issues
    pub issues: Vec<CohesionIssue>,
    /// Total issue count
    pub issues_count: usize,
    /// Number of files analyzed
    pub files_analyzed: usize,
    /// Average cohesion score across all files
    pub average_cohesion: f64,
    /// Average doc alignment score (files with docs only)
    pub average_doc_alignment: f64,
}

impl Default for CohesionAnalysisResults {
    fn default() -> Self {
        Self {
            enabled: false,
            file_scores: HashMap::new(),
            folder_scores: HashMap::new(),
            issues: Vec::new(),
            issues_count: 0,
            files_analyzed: 0,
            average_cohesion: 1.0,
            average_doc_alignment: 1.0,
        }
    }
}

/// Cohesion score for a single file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCohesionScore {
    /// File path
    pub path: PathBuf,
    /// Cohesion score (0.0 = low cohesion, 1.0 = high cohesion)
    pub cohesion: f64,
    /// Doc-code alignment score (None if no module doc)
    pub doc_alignment: Option<f64>,
    /// Number of entities in file
    pub entity_count: usize,
    /// Outlier entities (low similarity to file centroid)
    pub outliers: Vec<EntityOutlier>,
    /// Roll-up state: count of child embeddings
    pub rollup_n: usize,
    /// Roll-up state: sum of normalized embeddings (for folder aggregation)
    #[serde(skip)]
    pub rollup_sum: Option<Vec<f32>>,
}

/// Cohesion score for a folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderCohesionScore {
    /// Folder path
    pub path: PathBuf,
    /// Cohesion score across all files in folder
    pub cohesion: f64,
    /// Doc-code alignment score (None if no README)
    pub doc_alignment: Option<f64>,
    /// Number of files in folder
    pub file_count: usize,
    /// Outlier files (low similarity to folder centroid)
    pub outliers: Vec<FileOutlier>,
}

/// An entity that is a semantic outlier within its file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityOutlier {
    /// Entity name
    pub name: String,
    /// Entity kind (function, class, etc.)
    pub kind: String,
    /// Line range
    pub line_range: Option<(usize, usize)>,
    /// Similarity to file centroid
    pub similarity: f64,
    /// Similarity to file's doc embedding (if available)
    pub doc_similarity: Option<f64>,
}

/// A file that is a semantic outlier within its folder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOutlier {
    /// File path (relative to folder)
    pub path: PathBuf,
    /// Similarity to folder centroid
    pub similarity: f64,
}

/// A cohesion-related issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohesionIssue {
    /// Issue code (e.g., "DOC_MISMATCH", "SEMANTIC_OUTLIER")
    pub code: String,
    /// Issue category
    pub category: String,
    /// Affected path (file or folder)
    pub path: PathBuf,
    /// Affected entity name (if entity-level issue)
    pub entity: Option<String>,
    /// Line range (if entity-level issue)
    pub line_range: Option<(usize, usize)>,
    /// Severity (0.0 to 1.0)
    pub severity: f64,
    /// Human-readable description
    pub description: String,
}

/// Issue codes for cohesion analysis
pub mod issue_codes {
    /// Documentation doesn't match code semantics
    pub const DOC_MISMATCH: &str = "COH001";
    /// Documentation is too short to be meaningful
    pub const DOC_TOO_SHORT: &str = "COH002";
    /// Documentation is too generic (low specificity)
    pub const DOC_GENERIC: &str = "COH003";
    /// Entity is a semantic outlier in its container
    pub const SEMANTIC_OUTLIER: &str = "COH004";
    /// Entity doesn't match container's documented intent
    pub const DOC_OUTLIER: &str = "COH005";
    /// File/folder has low semantic cohesion
    pub const LOW_COHESION: &str = "COH006";
}

/// Main cohesion analysis extractor
pub struct CohesionExtractor {
    config: CohesionConfig,
    embedding_provider: Option<EmbeddingProvider>,
    calculator: CohesionCalculator,
    features: Vec<FeatureDefinition>,
}

impl CohesionExtractor {
    /// Create a new cohesion extractor with default configuration
    pub fn new() -> Self {
        Self::with_config(CohesionConfig::default())
    }

    /// Create a new cohesion extractor with the given configuration
    pub fn with_config(config: CohesionConfig) -> Self {
        let calculator = CohesionCalculator::new(&config);

        let mut extractor = Self {
            config,
            embedding_provider: None,
            calculator,
            features: Vec::new(),
        };

        extractor.initialize_features();
        extractor
    }

    /// Initialize the embedding provider (lazy initialization)
    pub fn ensure_embedding_provider(&mut self) -> Result<()> {
        if self.embedding_provider.is_none() {
            self.embedding_provider = Some(EmbeddingProvider::new(&self.config.embedding)?);
        }
        Ok(())
    }

    fn initialize_features(&mut self) {
        self.features = vec![
            FeatureDefinition::new(
                "semantic_cohesion",
                "Semantic cohesion score (0=scattered topics, 1=unified topic)",
            ),
            FeatureDefinition::new(
                "doc_alignment",
                "Documentation-code alignment score (0=mismatch, 1=aligned)",
            ),
            FeatureDefinition::new(
                "outlier_ratio",
                "Ratio of semantic outliers to total entities",
            ),
        ];
    }

    /// Analyze cohesion for a set of files with pre-read source content
    pub async fn analyze_with_sources(
        &mut self,
        file_sources: &[(PathBuf, String)],
        root_path: &Path,
    ) -> Result<CohesionAnalysisResults> {
        use tracing::info;

        if !self.config.enabled {
            return Ok(CohesionAnalysisResults::default());
        }

        info!("Starting cohesion analysis for {} files", file_sources.len());
        self.ensure_embedding_provider()?;

        let embedding_provider = self.embedding_provider.as_ref().unwrap();
        let dimension = embedding_provider.dimension();

        // Phase 1: Extract entities and build TF-IDF corpus
        let mut tfidf = symbols::TfIdfCalculator::new(self.config.symbols.clone());
        let mut all_file_entities: HashMap<PathBuf, Vec<extractor::CohesionEntity>> = HashMap::new();

        for (path, source) in file_sources {
            let Some(lang) = self.detect_language(path) else { continue };
            let Ok(mut entity_extractor) = extractor::CohesionEntityExtractor::new(&lang) else { continue };
            let Ok(entities) = entity_extractor.extract_entities(source, path) else { continue };

            for entity in &entities {
                tfidf.add_document(&Self::collect_entity_symbols(entity));
            }
            all_file_entities.insert(path.clone(), entities);
        }

        info!("Extracted entities from {} files, {} in TF-IDF corpus",
              all_file_entities.len(), tfidf.total_documents());

        // Phase 2: Generate embeddings for each entity
        let mut file_scores: HashMap<PathBuf, FileCohesionScore> = HashMap::new();
        let mut all_issues: Vec<CohesionIssue> = Vec::new();
        let mut folder_rollups: HashMap<PathBuf, metrics::RollupState> = HashMap::new();

        for (path, entities) in &all_file_entities {
            if entities.is_empty() {
                continue;
            }

            // Build code text for each entity using TF-IDF filtered symbols
            let entity_texts: Vec<String> = entities
                .iter()
                .map(|entity| Self::build_entity_embedding_text(entity, &tfidf))
                .collect();

            // Generate embeddings in batch
            let embeddings = match embedding_provider.embed_batch(&entity_texts) {
                Ok(e) => e,
                Err(_) => continue,
            };

            if embeddings.is_empty() {
                continue;
            }

            // Calculate file cohesion
            let cohesion = self.calculator.cohesion_score(&embeddings);

            // Get module docstring embedding for doc alignment
            let doc_alignment = if let Some((_, source)) = file_sources.iter().find(|(p, _)| p == path) {
                self.calculate_doc_alignment(path, source, &embeddings, embedding_provider)
            } else {
                None
            };

            // Find outliers
            let mut outliers = Vec::new();
            if let Some(centroid) = self.calculator.robust_centroid(&embeddings) {
                let outlier_indices = self.calculator.find_outliers(
                    &embeddings,
                    &centroid,
                    self.config.thresholds.outlier_percentile,
                    self.config.thresholds.min_outlier_similarity,
                );

                for (idx, similarity) in outlier_indices {
                    if idx < entities.len() {
                        let entity = &entities[idx];
                        outliers.push(EntityOutlier {
                            name: entity.name.clone(),
                            kind: entity.kind.clone(),
                            line_range: Some(entity.line_range),
                            similarity,
                            doc_similarity: None,
                        });

                        // Generate issue for significant outliers
                        if similarity < self.config.thresholds.min_outlier_similarity {
                            all_issues.push(Self::create_outlier_issue(path, entity, similarity));
                        }
                    }
                }
            }

            // Generate issue for low cohesion files
            if cohesion < self.config.thresholds.min_cohesion && entities.len() >= self.config.rollup.min_file_entities {
                all_issues.push(Self::create_low_cohesion_issue(path, cohesion));
            }

            // Generate issue for doc mismatch
            if let Some(alignment) = doc_alignment {
                if alignment < self.config.thresholds.min_doc_alignment {
                    all_issues.push(Self::create_doc_mismatch_issue(path, alignment));
                }
            }

            // Build rollup state for folder aggregation
            let mut rollup = metrics::RollupState::new(dimension);
            for emb in &embeddings {
                rollup.add(emb);
            }

            file_scores.insert(path.clone(), FileCohesionScore {
                path: path.clone(),
                cohesion,
                doc_alignment,
                entity_count: entities.len(),
                outliers,
                rollup_n: rollup.n,
                rollup_sum: Some(rollup.sum.clone()),
            });

            // Aggregate to all ancestor folders
            let weight = self.calculator.file_weight(entities.len()) as f32;
            let mut current = path.parent();
            while let Some(parent) = current {
                // Stop at reasonable boundaries (don't go above src-like directories)
                let folder_name = parent.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if folder_name.is_empty() || parent.as_os_str().is_empty() {
                    break;
                }

                let folder_path = parent.to_path_buf();
                folder_rollups
                    .entry(folder_path)
                    .or_insert_with(|| metrics::RollupState::new(dimension))
                    .add_rollup(&rollup, weight);

                current = parent.parent();
            }
        }

        // Phase 3: Calculate folder-level cohesion
        let mut folder_scores: HashMap<PathBuf, FolderCohesionScore> = HashMap::new();

        for (folder_path, rollup) in &folder_rollups {
            // Count all files under this folder (recursive)
            let files_in_folder: Vec<&FileCohesionScore> = file_scores.values()
                .filter(|f| f.path.starts_with(folder_path))
                .collect();

            if files_in_folder.len() < self.config.rollup.min_folder_files {
                continue;
            }

            let folder_cohesion = rollup.cohesion();

            // Find file outliers within folder
            let mut file_outliers = Vec::new();
            let centroid = rollup.centroid();

            for file_score in &files_in_folder {
                if let Some(ref sum) = file_score.rollup_sum {
                    let file_centroid = metrics::normalize(sum);
                    let similarity = metrics::cosine_similarity(&file_centroid, &centroid);

                    if similarity < self.config.thresholds.min_outlier_similarity {
                        file_outliers.push(FileOutlier {
                            path: file_score.path.strip_prefix(folder_path).unwrap_or(&file_score.path).to_path_buf(),
                            similarity,
                        });
                    }
                }
            }

            folder_scores.insert(folder_path.clone(), FolderCohesionScore {
                path: folder_path.clone(),
                cohesion: folder_cohesion,
                doc_alignment: None, // TODO: README analysis
                file_count: files_in_folder.len(),
                outliers: file_outliers,
            });
        }

        // Calculate averages
        let avg_cohesion = if file_scores.is_empty() {
            1.0
        } else {
            file_scores.values().map(|f| f.cohesion).sum::<f64>() / file_scores.len() as f64
        };

        let doc_scores: Vec<f64> = file_scores.values()
            .filter_map(|f| f.doc_alignment)
            .collect();
        let avg_doc_alignment = if doc_scores.is_empty() {
            1.0
        } else {
            doc_scores.iter().sum::<f64>() / doc_scores.len() as f64
        };

        let issues_count = all_issues.len();

        info!("Cohesion analysis complete: {} files, {} folders, {} issues, avg cohesion: {:.2}",
              file_scores.len(), folder_scores.len(), issues_count, avg_cohesion);

        Ok(CohesionAnalysisResults {
            enabled: true,
            file_scores: file_scores.clone(),
            folder_scores,
            issues: all_issues,
            issues_count,
            files_analyzed: file_scores.len(),
            average_cohesion: avg_cohesion,
            average_doc_alignment: avg_doc_alignment,
        })
    }

    /// Calculate doc-code alignment for a file
    fn calculate_doc_alignment(
        &self,
        path: &Path,
        source: &str,
        code_embeddings: &[Vec<f32>],
        embedding_provider: &embeddings::EmbeddingProvider,
    ) -> Option<f64> {
        let lang = self.detect_language(path)?;
        let mut extractor = extractor::CohesionEntityExtractor::new(&lang).ok()?;
        let module_doc = extractor.extract_module_docstring(source)?;

        if module_doc.split_whitespace().count() < self.config.thresholds.min_doc_tokens {
            return None; // Doc too short to be meaningful
        }

        let doc_embedding = embedding_provider.embed_one(&module_doc).ok()?;

        // Calculate centroid of code embeddings
        let code_centroid = self.calculator.robust_centroid(code_embeddings)?;

        Some(self.calculator.doc_alignment(&doc_embedding, &code_centroid))
    }

    /// Detect language from file extension
    fn detect_language(&self, path: &Path) -> Option<String> {
        const LANG_EXTENSIONS: &[(&str, &str)] = &[
            ("py", "python"), ("rs", "rust"), ("js", "javascript"),
            ("jsx", "javascript"), ("mjs", "javascript"),
            ("ts", "typescript"), ("tsx", "typescript"), ("go", "go"),
        ];
        let ext = path.extension()?.to_str()?;
        LANG_EXTENSIONS.iter()
            .find(|(e, _)| *e == ext)
            .map(|(_, lang)| lang.to_string())
    }

    /// Create an issue for a semantic outlier entity
    fn create_outlier_issue(
        path: &Path,
        entity: &extractor::CohesionEntity,
        similarity: f64,
    ) -> CohesionIssue {
        CohesionIssue {
            code: issue_codes::SEMANTIC_OUTLIER.to_string(),
            category: "cohesion".to_string(),
            path: path.to_path_buf(),
            entity: Some(entity.name.clone()),
            line_range: Some(entity.line_range),
            severity: 1.0 - similarity,
            description: format!(
                "{} '{}' appears to be semantically unrelated to the rest of the file (similarity: {:.2})",
                entity.kind, entity.name, similarity
            ),
        }
    }

    /// Create an issue for a low cohesion file
    fn create_low_cohesion_issue(path: &Path, cohesion: f64) -> CohesionIssue {
        CohesionIssue {
            code: issue_codes::LOW_COHESION.to_string(),
            category: "cohesion".to_string(),
            path: path.to_path_buf(),
            entity: None,
            line_range: None,
            severity: 1.0 - cohesion,
            description: format!(
                "File has low semantic cohesion ({:.2}). Consider splitting into more focused modules.",
                cohesion
            ),
        }
    }

    /// Collect all symbols from an entity for TF-IDF processing.
    fn collect_entity_symbols(entity: &extractor::CohesionEntity) -> Vec<String> {
        entity.symbols.name_tokens.iter()
            .chain(entity.symbols.signature_tokens.iter())
            .chain(entity.symbols.referenced_symbols.iter())
            .cloned()
            .collect()
    }

    /// Build embedding text from entity symbols using TF-IDF filtering.
    fn build_entity_embedding_text(
        entity: &extractor::CohesionEntity,
        tfidf: &symbols::TfIdfCalculator,
    ) -> String {
        let all_symbols = Self::collect_entity_symbols(entity);
        let top_symbols = tfidf.select_top_symbols(&all_symbols);
        let mut text_parts = vec![entity.symbols.kind.clone(), entity.qualified_name.clone()];
        text_parts.extend(top_symbols);
        text_parts.join(" ")
    }

    /// Create an issue for documentation mismatch
    fn create_doc_mismatch_issue(path: &Path, alignment: f64) -> CohesionIssue {
        CohesionIssue {
            code: issue_codes::DOC_MISMATCH.to_string(),
            category: "documentation".to_string(),
            path: path.to_path_buf(),
            entity: None,
            line_range: None,
            severity: 1.0 - alignment,
            description: format!(
                "Module documentation doesn't align with code semantics ({:.2}). Consider updating the docstring.",
                alignment
            ),
        }
    }

    /// Analyze cohesion for a set of files (legacy interface - reads files)
    pub async fn analyze(
        &mut self,
        files: &[PathBuf],
        root_path: &Path,
    ) -> Result<CohesionAnalysisResults> {
        if !self.config.enabled {
            return Ok(CohesionAnalysisResults::default());
        }

        // Read file contents
        let mut file_sources: Vec<(PathBuf, String)> = Vec::new();
        for path in files {
            if let Ok(content) = std::fs::read_to_string(path) {
                file_sources.push((path.clone(), content));
            }
        }

        self.analyze_with_sources(&file_sources, root_path).await
    }
}

impl Default for CohesionExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl FeatureExtractor for CohesionExtractor {
    fn name(&self) -> &str {
        "cohesion"
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

        // Default values when cohesion analysis is disabled or not yet computed
        features.insert("semantic_cohesion".to_string(), 1.0);
        features.insert("doc_alignment".to_string(), 1.0);
        features.insert("outlier_ratio".to_string(), 0.0);

        Ok(features)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cohesion_extractor_creates_with_default_config() {
        let extractor = CohesionExtractor::new();
        assert_eq!(extractor.name(), "cohesion");
        assert_eq!(extractor.features().len(), 3);
    }

    #[test]
    fn cohesion_results_default_is_disabled() {
        let results = CohesionAnalysisResults::default();
        assert!(!results.enabled);
        assert!(results.issues.is_empty());
    }

    #[test]
    fn issue_codes_are_defined() {
        assert_eq!(issue_codes::DOC_MISMATCH, "COH001");
        assert_eq!(issue_codes::SEMANTIC_OUTLIER, "COH004");
        assert_eq!(issue_codes::LOW_COHESION, "COH006");
    }
}
