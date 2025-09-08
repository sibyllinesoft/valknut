//! Name generation, behavior extraction, and lexicon management for semantic naming analysis.

use std::collections::HashMap;

use crate::core::errors::Result;

use super::config::*;

/// Behavior extractor using static analysis
pub struct BehaviorExtractor {
    config: NamesConfig,
}

impl BehaviorExtractor {
    pub fn new(config: &NamesConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn extract_behavior(&self, func: &FunctionInfo) -> Result<BehaviorSignature> {
        // TODO: Implement AST-based behavior extraction
        // For now, return a basic signature based on name heuristics
        let name_lower = func.name.to_lowercase();
        
        let side_effects = SideEffects {
            http_operations: name_lower.contains("fetch") || name_lower.contains("request"),
            database_operations: DatabaseOperations {
                reads: name_lower.starts_with("get_") || name_lower.starts_with("find_"),
                writes: name_lower.starts_with("update_") || name_lower.starts_with("set_"),
                creates: name_lower.starts_with("create_") || name_lower.starts_with("insert_"),
                deletes: name_lower.starts_with("delete_") || name_lower.starts_with("remove_"),
            },
            file_operations: FileOperations {
                reads: name_lower.contains("read") || name_lower.contains("load"),
                writes: name_lower.contains("write") || name_lower.contains("save"),
                creates: name_lower.contains("create"),
                deletes: name_lower.contains("delete"),
            },
            network_operations: name_lower.contains("send") || name_lower.contains("receive"),
            console_output: name_lower.contains("print") || name_lower.contains("log"),
        };

        let execution_pattern = if name_lower.contains("async") {
            ExecutionPattern::Asynchronous
        } else {
            ExecutionPattern::Synchronous
        };

        let return_type = ReturnTypeInfo {
            primary_type: func.return_type.clone(),
            optional: name_lower.starts_with("find_") || name_lower.starts_with("try_"),
            collection: name_lower.contains("list") || name_lower.ends_with("s"),
            lazy_evaluation: name_lower.contains("iter") || name_lower.contains("stream"),
            type_category: TypeCategory::Object, // Default assumption
        };

        Ok(BehaviorSignature {
            side_effects,
            mutations: MutationPattern::Pure, // Default assumption
            execution_pattern,
            return_type,
            resource_handling: ResourceHandling {
                acquires_resources: false,
                releases_resources: false,
                returns_handles: false,
            },
            confidence: 0.7, // Heuristic-based confidence
        })
    }
}

/// Name generator using deterministic rules
pub struct NameGenerator {
    config: NamesConfig,
    verb_map: HashMap<String, Vec<String>>,
}

impl NameGenerator {
    pub fn new(config: &NamesConfig) -> Self {
        let mut verb_map = HashMap::new();
        
        // HTTP operations
        verb_map.insert("http_get".to_string(), vec!["fetch".to_string(), "get".to_string()]);
        
        // Database operations
        verb_map.insert("db_read".to_string(), vec!["get".to_string(), "find".to_string()]);
        verb_map.insert("db_write".to_string(), vec!["create".to_string(), "insert".to_string(), "update".to_string(), "upsert".to_string(), "delete".to_string()]);
        
        // Processing operations
        verb_map.insert("parse".to_string(), vec!["parse".to_string(), "deserialize".to_string()]);
        verb_map.insert("format".to_string(), vec!["format".to_string(), "serialize".to_string()]);
        verb_map.insert("validate".to_string(), vec!["validate".to_string(), "check".to_string()]);
        
        // Cache operations
        verb_map.insert("cache_lookup".to_string(), vec!["get_cached".to_string()]);
        
        // Iterator operations
        verb_map.insert("iterator".to_string(), vec!["iter".to_string(), "list".to_string()]);

        Self {
            config: config.clone(),
            verb_map,
        }
    }

    pub fn generate_proposals(
        &self,
        behavior: &BehaviorSignature,
        current_name: &str,
        lexicon: &ProjectLexicon,
    ) -> Result<Vec<NameProposal>> {
        let mut proposals = Vec::new();

        // Generate verb based on behavior
        let verb = self.select_verb(behavior)?;
        
        // Generate noun based on return type and context
        let noun = self.select_noun(behavior, current_name, lexicon)?;
        
        // Generate qualifiers
        let qualifiers = self.generate_qualifiers(behavior, current_name)?;

        // Construct name variants
        let base_name = format!("{}_{}", verb, noun);
        let base_proposal = NameProposal {
            name: self.apply_naming_convention(&base_name, &qualifiers)?,
            rationale: format!("Based on {} operation returning {}", verb, noun),
            confidence: 0.8,
            components: NameComponents {
                verb: verb.clone(),
                noun: noun.clone(),
                qualifiers: qualifiers.clone(),
            },
        };
        proposals.push(base_proposal);

        // Generate alternative with different verb if applicable
        if let Some(alt_verb) = self.get_alternative_verb(behavior) {
            let alt_name = format!("{}_{}", alt_verb, noun);
            let alt_proposal = NameProposal {
                name: self.apply_naming_convention(&alt_name, &qualifiers)?,
                rationale: format!("Alternative {} operation", alt_verb),
                confidence: 0.6,
                components: NameComponents {
                    verb: alt_verb,
                    noun: noun.clone(),
                    qualifiers: qualifiers.clone(),
                },
            };
            proposals.push(alt_proposal);
        }

        // Sort by confidence (highest first)
        proposals.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        
        Ok(proposals.into_iter().take(3).collect()) // Return top 3
    }

    fn select_verb(&self, behavior: &BehaviorSignature) -> Result<String> {
        // Select verb based on primary side effect
        if behavior.side_effects.database_operations.creates {
            Ok("create".to_string())
        } else if behavior.side_effects.database_operations.writes {
            Ok("update".to_string())
        } else if behavior.side_effects.database_operations.deletes {
            Ok("delete".to_string())
        } else if behavior.side_effects.database_operations.reads {
            if behavior.return_type.optional {
                Ok("find".to_string())
            } else {
                Ok("get".to_string())
            }
        } else if behavior.side_effects.http_operations {
            Ok("fetch".to_string())
        } else if behavior.side_effects.file_operations.reads {
            Ok("load".to_string())
        } else if behavior.side_effects.file_operations.writes {
            Ok("save".to_string())
        } else if behavior.return_type.collection {
            Ok("list".to_string())
        } else {
            Ok("process".to_string()) // Default fallback
        }
    }

    fn select_noun(&self, behavior: &BehaviorSignature, current_name: &str, lexicon: &ProjectLexicon) -> Result<String> {
        // Try to extract from return type first
        if let Some(ref return_type) = behavior.return_type.primary_type {
            return Ok(return_type.to_lowercase());
        }

        // Extract from current name
        let parts: Vec<&str> = current_name.split('_').collect();
        if parts.len() > 1 {
            let potential_noun = parts[1];
            if lexicon.domain_nouns.contains_key(potential_noun) {
                return Ok(potential_noun.to_string());
            }
        }

        // Fallback to generic names
        if behavior.return_type.collection {
            Ok("items".to_string())
        } else {
            Ok("data".to_string())
        }
    }

    fn generate_qualifiers(&self, behavior: &BehaviorSignature, current_name: &str) -> Result<Vec<String>> {
        let mut qualifiers = Vec::new();

        // Add qualifiers from current name
        let parts: Vec<&str> = current_name.split('_').collect();
        if parts.len() > 2 {
            for part in &parts[2..] {
                if part.starts_with("by_") || part.starts_with("with_") || part.starts_with("from_") {
                    qualifiers.push(part.to_string());
                }
            }
        }

        // Add async qualifier if needed
        if matches!(behavior.execution_pattern, ExecutionPattern::Asynchronous) {
            qualifiers.push("async".to_string());
        }

        Ok(qualifiers)
    }

    fn apply_naming_convention(&self, base_name: &str, qualifiers: &[String]) -> Result<String> {
        let mut name = base_name.to_string();
        
        // Add qualifiers
        for qualifier in qualifiers {
            name.push('_');
            name.push_str(qualifier);
        }

        // Apply abbreviation expansion
        let parts: Vec<String> = name.split('_').map(|part| {
            if let Some(expanded) = self.config.abbrev_map.get(part) {
                expanded.clone()
            } else {
                part.to_string()
            }
        }).collect();

        let final_name = parts.join("_");

        // Truncate if too long
        if final_name.len() > 40 {
            let truncated = parts.into_iter().take(3).collect::<Vec<_>>().join("_");
            Ok(truncated)
        } else {
            Ok(final_name)
        }
    }

    fn get_alternative_verb(&self, behavior: &BehaviorSignature) -> Option<String> {
        if behavior.side_effects.database_operations.reads {
            Some("retrieve".to_string())
        } else if behavior.side_effects.database_operations.writes {
            Some("modify".to_string())
        } else {
            None
        }
    }
}