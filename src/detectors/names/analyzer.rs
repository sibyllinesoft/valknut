//! Main analysis logic and mismatch detection for semantic naming analysis.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tracing::{debug, info, warn};

use crate::core::errors::{Result, ValknutError};
use crate::detectors::embedding::EmbeddingBackend;

use super::config::*;
use super::generator::{BehaviorExtractor, NameGenerator};

/// Main semantic naming analyzer
pub struct SemanticNameAnalyzer {
    config: NamesConfig,
    embedding_backend: Arc<EmbeddingBackend>,
    behavior_extractor: BehaviorExtractor,
    name_generator: NameGenerator,
    lexicon: ProjectLexicon,
}

impl SemanticNameAnalyzer {
    /// Create new semantic name analyzer
    pub async fn new(config: NamesConfig) -> Result<Self> {
        let embedding_backend = Arc::new(EmbeddingBackend::new(&config.embedding_model).await?);
        let behavior_extractor = BehaviorExtractor::new(&config);
        let name_generator = NameGenerator::new(&config);
        let lexicon = ProjectLexicon::new();

        Ok(Self {
            config,
            embedding_backend,
            behavior_extractor,
            name_generator,
            lexicon,
        })
    }

    /// Analyze functions for semantic naming issues
    pub async fn analyze_functions(
        &mut self,
        functions: &[FunctionInfo],
    ) -> Result<AnalysisResults> {
        info!("Starting semantic naming analysis for {} functions", functions.len());

        let mut rename_packs = Vec::new();
        let mut contract_packs = Vec::new();

        // Build project lexicon from all functions
        self.build_lexicon(functions)?;

        // Analyze each function
        for func in functions {
            if let Some(result) = self.analyze_function(func).await? {
                match result {
                    AnalysisResult::RenamePack(pack) => rename_packs.push(pack),
                    AnalysisResult::ContractMismatchPack(pack) => contract_packs.push(pack),
                }
            }
        }

        // Sort by priority (highest first)
        rename_packs.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());
        contract_packs.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());

        Ok(AnalysisResults {
            rename_packs,
            contract_mismatch_packs: contract_packs,
            lexicon_consistency: self.check_lexicon_consistency(),
        })
    }

    /// Analyze a single function for naming issues
    async fn analyze_function(&self, func: &FunctionInfo) -> Result<Option<AnalysisResult>> {
        debug!("Analyzing function: {}", func.name);

        // Extract behavior signature
        let behavior = self.behavior_extractor.extract_behavior(func)?;
        
        // Skip if confidence is too low
        if behavior.confidence < 0.5 {
            debug!("Skipping {} due to low behavior confidence: {}", func.name, behavior.confidence);
            return Ok(None);
        }

        // Check semantic mismatch
        let mismatch = self.check_semantic_mismatch(&func.name, &behavior).await?;
        
        // Apply gating thresholds
        if mismatch.mismatch_score < self.config.min_mismatch {
            debug!("Mismatch score {} below threshold {}", mismatch.mismatch_score, self.config.min_mismatch);
            return Ok(None);
        }

        // Get impact analysis
        let impact = self.analyze_impact(func)?;
        
        if impact.external_refs < self.config.min_impact {
            debug!("Impact {} below threshold {}", impact.external_refs, self.config.min_impact);
            return Ok(None);
        }

        // Protect public API if configured
        if self.config.protect_public_api && impact.public_api {
            debug!("Skipping public API function: {}", func.name);
            return Ok(None);
        }

        // Generate recommendations
        let result = if self.should_generate_contract_pack(&mismatch) {
            let solutions = self.generate_contract_solutions(&func.name, &behavior, &mismatch)?;
            let priority = self.calculate_contract_priority(&mismatch, &impact);
            
            AnalysisResult::ContractMismatchPack(ContractMismatchPack {
                function_id: func.id.clone(),
                current_name: func.name.clone(),
                file_path: func.file_path.clone(),
                line_number: func.line_number,
                contract_issues: self.extract_contract_issues(&mismatch),
                solutions,
                impact,
                priority,
            })
        } else {
            let proposals = self.name_generator.generate_proposals(&behavior, &func.name, &self.lexicon)?;
            let priority = self.calculate_rename_priority(&mismatch, &impact);
            
            AnalysisResult::RenamePack(RenamePack {
                function_id: func.id.clone(),
                current_name: func.name.clone(),
                file_path: func.file_path.clone(),
                line_number: func.line_number,
                proposals,
                impact,
                mismatch,
                priority,
            })
        };

        Ok(Some(result))
    }

    /// Check semantic mismatch between name and behavior
    async fn check_semantic_mismatch(
        &self,
        name: &str,
        behavior: &BehaviorSignature,
    ) -> Result<SemanticMismatch> {
        // Generate glosses for name and behavior
        let name_gloss = self.generate_name_gloss(name)?;
        let behavior_gloss = self.generate_behavior_gloss(behavior)?;

        // Compute cosine similarity using embeddings
        let cosine_similarity = self.embedding_backend
            .cosine_similarity(&name_gloss, &behavior_gloss)
            .await?;

        // Detect specific mismatch types using rules
        let mismatch_types = self.detect_mismatch_types(name, behavior)?;

        // Calculate overall mismatch score
        let mismatch_score = self.calculate_mismatch_score(cosine_similarity, &mismatch_types, behavior);

        // Apply confidence dampers
        let mut confidence = 1.0;
        if behavior.confidence < 0.8 {
            confidence -= 0.15; // Weak behavior inference
        }
        if name.split('_').count() < 2 {
            confidence -= 0.1; // Short name
        }
        confidence = confidence.max(0.0);

        Ok(SemanticMismatch {
            cosine_similarity,
            mismatch_types,
            mismatch_score,
            confidence,
        })
    }

    /// Calculate mismatch score using TODO.md formula
    fn calculate_mismatch_score(
        &self,
        cosine_similarity: f64,
        mismatch_types: &[MismatchType],
        behavior: &BehaviorSignature,
    ) -> f64 {
        // Base score from cosine similarity
        let semantic_component = 0.5 * (1.0 - cosine_similarity);

        // Effect mismatch component
        let effect_component = 0.2 * if mismatch_types.iter().any(|m| matches!(m, MismatchType::EffectMismatch { .. })) { 1.0 } else { 0.0 };

        // Cardinality mismatch component
        let cardinality_component = 0.1 * if mismatch_types.iter().any(|m| matches!(m, MismatchType::CardinalityMismatch { .. })) { 1.0 } else { 0.0 };

        // Optionality mismatch component
        let optionality_component = 0.1 * if mismatch_types.iter().any(|m| matches!(m, MismatchType::OptionalityMismatch { .. })) { 1.0 } else { 0.0 };

        // Async/idempotence mismatch component
        let async_component = 0.1 * if mismatch_types.iter().any(|m| matches!(m, MismatchType::AsyncMismatch { .. })) { 1.0 } else { 0.0 };

        semantic_component + effect_component + cardinality_component + optionality_component + async_component
    }

    /// Generate name gloss for embedding
    fn generate_name_gloss(&self, name: &str) -> Result<String> {
        // Split name into components and expand abbreviations
        let mut components = Vec::new();
        
        for part in name.split('_').filter(|s| !s.is_empty()) {
            if let Some(expanded) = self.config.abbrev_map.get(part) {
                components.push(expanded.clone());
            } else if self.config.allowed_abbrevs.contains(&part.to_string()) {
                components.push(part.to_string());
            } else {
                components.push(part.to_string());
            }
        }

        Ok(components.join(" "))
    }

    /// Generate behavior gloss for embedding
    fn generate_behavior_gloss(&self, behavior: &BehaviorSignature) -> Result<String> {
        let mut gloss_parts = Vec::new();

        // Add effects
        if behavior.side_effects.database_operations.reads {
            gloss_parts.push("reads database");
        }
        if behavior.side_effects.database_operations.writes {
            gloss_parts.push("writes database");
        }
        if behavior.side_effects.database_operations.creates {
            gloss_parts.push("creates database record");
        }
        if behavior.side_effects.database_operations.deletes {
            gloss_parts.push("deletes database record");
        }
        if behavior.side_effects.http_operations {
            gloss_parts.push("makes HTTP request");
        }
        if behavior.side_effects.file_operations.reads {
            gloss_parts.push("reads file");
        }
        if behavior.side_effects.file_operations.writes {
            gloss_parts.push("writes file");
        }

        // Add execution pattern
        match behavior.execution_pattern {
            ExecutionPattern::Asynchronous => gloss_parts.push("asynchronous operation"),
            ExecutionPattern::Synchronous => gloss_parts.push("synchronous operation"),
            ExecutionPattern::Ambiguous => gloss_parts.push("flexible execution"),
        }

        // Add return type info
        if let Some(ref return_type) = behavior.return_type.primary_type {
            gloss_parts.push(&format!("returns {}", return_type));
        }
        if behavior.return_type.optional {
            gloss_parts.push("optional return");
        }
        if behavior.return_type.collection {
            gloss_parts.push("returns collection");
        }

        Ok(gloss_parts.join(" "))
    }

    /// Detect specific mismatch types using rule-based analysis
    fn detect_mismatch_types(&self, name: &str, behavior: &BehaviorSignature) -> Result<Vec<MismatchType>> {
        let mut mismatches = Vec::new();
        let name_lower = name.to_lowercase();

        // Effect mismatch detection
        if name_lower.starts_with("get_") || name_lower.starts_with("is_") {
            // Name implies read-only operation
            if behavior.side_effects.database_operations.writes || 
               behavior.side_effects.database_operations.creates ||
               behavior.side_effects.database_operations.deletes ||
               behavior.side_effects.file_operations.writes {
                mismatches.push(MismatchType::EffectMismatch {
                    expected: "read-only operation".to_string(),
                    actual: "modifies state".to_string(),
                });
            }
        }

        // Cardinality mismatch
        if behavior.return_type.collection && !name_lower.contains("list") && 
           !name_lower.ends_with("s") && !name_lower.contains("all") {
            mismatches.push(MismatchType::CardinalityMismatch {
                expected: "single item".to_string(),
                actual: "collection".to_string(),
            });
        }

        // Optionality mismatch
        if (name_lower.starts_with("find_") || name_lower.starts_with("try_")) && !behavior.return_type.optional {
            mismatches.push(MismatchType::OptionalityMismatch {
                expected: "optional return".to_string(),
                actual: "guaranteed return".to_string(),
            });
        }

        // Async mismatch
        match behavior.execution_pattern {
            ExecutionPattern::Asynchronous => {
                if !name_lower.contains("async") && !name_lower.ends_with("_async") {
                    mismatches.push(MismatchType::AsyncMismatch {
                        expected: "synchronous".to_string(),
                        actual: "asynchronous".to_string(),
                    });
                }
            },
            ExecutionPattern::Synchronous => {
                if name_lower.contains("async") || name_lower.ends_with("_async") {
                    mismatches.push(MismatchType::AsyncMismatch {
                        expected: "asynchronous".to_string(),
                        actual: "synchronous".to_string(),
                    });
                }
            },
            ExecutionPattern::Ambiguous => {} // No mismatch for ambiguous
        }

        Ok(mismatches)
    }

    /// Calculate priority for rename pack
    fn calculate_rename_priority(&self, mismatch: &SemanticMismatch, impact: &ImpactAnalysis) -> f64 {
        let value = mismatch.mismatch_score * (1.0 + (impact.external_refs as f64).ln());
        let effort = impact.effort_estimate as f64;
        value / (effort + 0.1) // Add epsilon to avoid division by zero
    }

    /// Calculate priority for contract mismatch pack
    fn calculate_contract_priority(&self, mismatch: &SemanticMismatch, impact: &ImpactAnalysis) -> f64 {
        let optionality_penalty = if mismatch.mismatch_types.iter().any(|m| matches!(m, MismatchType::OptionalityMismatch { .. })) { 0.3 } else { 0.0 };
        let cardinality_penalty = if mismatch.mismatch_types.iter().any(|m| matches!(m, MismatchType::CardinalityMismatch { .. })) { 0.2 } else { 0.0 };
        
        let value = mismatch.mismatch_score + optionality_penalty + cardinality_penalty;
        let effort = if impact.public_api { 2.0 } else { 1.0 } * impact.effort_estimate as f64;
        
        value / (effort + 0.1)
    }

    /// Build project lexicon from function information
    fn build_lexicon(&mut self, functions: &[FunctionInfo]) -> Result<()> {
        // Extract domain nouns from function names, types, and file paths
        let mut domain_nouns = HashMap::new();
        let mut verb_patterns = HashMap::new();

        for func in functions {
            // Extract nouns from function name
            for part in func.name.split('_') {
                if let Some(noun) = self.extract_noun_from_part(part) {
                    domain_nouns.entry(noun.clone())
                        .or_insert_with(|| DomainNoun {
                            canonical: noun,
                            variants: HashSet::new(),
                            contexts: Vec::new(),
                            frequency: 0,
                        })
                        .frequency += 1;
                }
            }

            // Extract verbs from function names
            if let Some(verb) = func.name.split('_').next() {
                verb_patterns.entry(verb.to_string())
                    .or_insert_with(|| VerbUsage {
                        verb: verb.to_string(),
                        typical_effects: HashSet::new(),
                        frequency: 0,
                    })
                    .frequency += 1;
            }
        }

        self.lexicon.domain_nouns = domain_nouns;
        self.lexicon.verb_patterns = verb_patterns;

        Ok(())
    }

    /// Extract noun from name part (simplified heuristic)
    fn extract_noun_from_part(&self, part: &str) -> Option<String> {
        // Skip common verbs and prepositions
        let skip_words = ["get", "set", "is", "has", "can", "should", "will", "create", "update", "delete", "find", "by", "with", "from", "to"];
        if skip_words.contains(&part) {
            return None;
        }

        // Return the part if it's likely a noun
        if part.len() > 2 {
            Some(part.to_string())
        } else {
            None
        }
    }

    /// Check if should generate contract mismatch pack vs rename pack
    fn should_generate_contract_pack(&self, mismatch: &SemanticMismatch) -> bool {
        // Generate contract pack if there are severe optionality or cardinality mismatches
        mismatch.mismatch_types.iter().any(|m| {
            matches!(m, MismatchType::OptionalityMismatch { .. } | MismatchType::CardinalityMismatch { .. })
        })
    }

    /// Generate contract solutions
    fn generate_contract_solutions(
        &self,
        name: &str,
        behavior: &BehaviorSignature,
        mismatch: &SemanticMismatch,
    ) -> Result<Vec<Solution>> {
        let mut solutions = Vec::new();

        // Always offer rename as a solution
        let proposed_names = self.name_generator.generate_proposals(behavior, name, &self.lexicon)?;
        if let Some(best_proposal) = proposed_names.first() {
            solutions.push(Solution::Rename {
                to_name: best_proposal.name.clone(),
                rationale: best_proposal.rationale.clone(),
            });
        }

        // Suggest contract changes for specific mismatch types
        for mismatch_type in &mismatch.mismatch_types {
            match mismatch_type {
                MismatchType::OptionalityMismatch { .. } => {
                    solutions.push(Solution::ContractChange {
                        description: "Make return type optional (Option<T>, nullable, etc.)".to_string(),
                        effort: 2,
                    });
                },
                MismatchType::CardinalityMismatch { .. } => {
                    solutions.push(Solution::ContractChange {
                        description: "Change return type to collection/iterator".to_string(),
                        effort: 3,
                    });
                },
                _ => {}, // Other mismatches better solved by renaming
            }
        }

        Ok(solutions)
    }

    /// Extract contract issues from mismatch
    fn extract_contract_issues(&self, mismatch: &SemanticMismatch) -> Vec<ContractIssue> {
        mismatch.mismatch_types.iter().map(|mismatch_type| {
            let (description, name_implies, actual_behavior, severity) = match mismatch_type {
                MismatchType::EffectMismatch { expected, actual } => (
                    "Function name implies different side effects than actual behavior".to_string(),
                    expected.clone(),
                    actual.clone(),
                    ContractSeverity::High,
                ),
                MismatchType::CardinalityMismatch { expected, actual } => (
                    "Function name implies different return cardinality than actual".to_string(),
                    expected.clone(),
                    actual.clone(),
                    ContractSeverity::Medium,
                ),
                MismatchType::OptionalityMismatch { expected, actual } => (
                    "Function name implies different return optionality than actual".to_string(),
                    expected.clone(),
                    actual.clone(),
                    ContractSeverity::High,
                ),
                MismatchType::AsyncMismatch { expected, actual } => (
                    "Function name implies different execution pattern than actual".to_string(),
                    expected.clone(),
                    actual.clone(),
                    ContractSeverity::Medium,
                ),
                MismatchType::OperationMismatch { expected, actual } => (
                    "Function name implies different operation type than actual".to_string(),
                    expected.clone(),
                    actual.clone(),
                    ContractSeverity::Medium,
                ),
            };

            ContractIssue {
                description,
                name_implies,
                actual_behavior,
                severity,
            }
        }).collect()
    }

    /// Analyze impact of changing a function
    fn analyze_impact(&self, func: &FunctionInfo) -> Result<ImpactAnalysis> {
        // This would normally use cross-reference analysis
        // For now, provide a simple implementation
        Ok(ImpactAnalysis {
            external_refs: func.call_sites.len(),
            affected_files: func.call_sites.iter().map(|cs| cs.file_path.as_str()).collect::<HashSet<_>>().len(),
            public_api: func.visibility == "public",
            effort_estimate: if func.visibility == "public" { 5 } else { 2 },
            affected_locations: func.call_sites.iter().map(|cs| format!("{}:{}", cs.file_path, cs.line_number)).collect(),
        })
    }

    /// Check lexicon consistency (detect synonym collisions)
    fn check_lexicon_consistency(&self) -> Vec<ConsistencyIssue> {
        // This would implement synonym detection using embeddings
        // For now, return empty
        Vec::new()
    }
}