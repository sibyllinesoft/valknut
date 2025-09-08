//! Simplified semantic naming analyzer using rule-based analysis.
//!
//! This module implements a deterministic semantic naming analysis system that:
//! - Extracts behavior signatures from code using AST analysis
//! - Uses rule-based semantic matching instead of embeddings
//! - Applies deterministic naming rules based on observed effects
//! - Generates rename recommendations and contract mismatch analysis
//! - Maintains project consistency through lexicon building

use std::collections::{HashMap, HashSet};
use std::path::Path;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::errors::{Result, ValknutError};

/// Configuration for semantic naming analysis (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamesConfig {
    /// Enable semantic naming analysis
    pub enabled: bool,
    /// Minimum mismatch score to trigger analysis (0.0-1.0)
    pub min_mismatch: f64,
    /// Minimum external references impact threshold
    pub min_impact: usize,
    /// Protect public API functions from aggressive renaming
    pub protect_public_api: bool,
    /// Abbreviation expansion mappings
    pub abbrev_map: HashMap<String, String>,
    /// Allowed abbreviations that don't need expansion
    pub allowed_abbrevs: Vec<String>,
}

impl Default for NamesConfig {
    fn default() -> Self {
        let mut abbrev_map = HashMap::new();
        abbrev_map.insert("usr".to_string(), "user".to_string());
        abbrev_map.insert("cfg".to_string(), "config".to_string());
        abbrev_map.insert("btn".to_string(), "button".to_string());
        abbrev_map.insert("mgr".to_string(), "manager".to_string());
        abbrev_map.insert("svc".to_string(), "service".to_string());
        abbrev_map.insert("impl".to_string(), "implementation".to_string());
        abbrev_map.insert("util".to_string(), "utility".to_string());
        abbrev_map.insert("calc".to_string(), "calculate".to_string());

        Self {
            enabled: true,
            min_mismatch: 0.65,
            min_impact: 3,
            protect_public_api: true,
            abbrev_map,
            allowed_abbrevs: vec![
                "id".to_string(), "url".to_string(), "db".to_string(),
                "io".to_string(), "api".to_string(), "ui".to_string(),
                "os".to_string(), "fs".to_string(),
            ],
        }
    }
}

/// Behavior signature extracted from static analysis (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSignature {
    /// Side effects detected
    pub side_effects: SideEffects,
    /// Return type characteristics
    pub return_type: ReturnTypeInfo,
    /// Async/synchronous execution pattern
    pub execution_pattern: ExecutionPattern,
    /// Confidence in behavior inference (0.0-1.0)
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffects {
    /// Has database operations
    pub has_database_ops: bool,
    /// Has file operations
    pub has_file_ops: bool,
    /// Has HTTP/network operations
    pub has_network_ops: bool,
    /// Has mutation operations
    pub has_mutations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnTypeInfo {
    /// Whether return can be null/None/undefined
    pub optional: bool,
    /// Whether returns a collection/iterator
    pub collection: bool,
    /// Scalar, object, or complex type
    pub type_category: TypeCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeCategory {
    Scalar,
    Object,
    Collection,
    Unit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionPattern {
    Synchronous,
    Asynchronous,
    Ambiguous,
}

/// Semantic mismatch between function name and behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMismatch {
    /// Rule-based similarity score between name and behavior (0.0-1.0)
    pub similarity_score: f64,
    /// Specific mismatch types detected
    pub mismatch_types: Vec<MismatchType>,
    /// Overall mismatch score (higher = more mismatched)
    pub mismatch_score: f64,
    /// Confidence in the mismatch detection
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MismatchType {
    EffectMismatch { expected: String, actual: String },
    CardinalityMismatch { expected: String, actual: String },
    OptionalityMismatch { expected: String, actual: String },
    AsyncMismatch { expected: String, actual: String },
    OperationMismatch { expected: String, actual: String },
}

/// Name proposal for a function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameProposal {
    /// Proposed function name
    pub name: String,
    /// Rationale for this name choice
    pub rationale: String,
    /// Confidence in this proposal (0.0-1.0)
    pub confidence: f64,
}

/// Naming analysis result for a single function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamingAnalysisResult {
    /// Function identifier
    pub function_id: String,
    /// Current function name
    pub current_name: String,
    /// File path where function is defined
    pub file_path: String,
    /// Line number of function definition
    pub line_number: usize,
    /// Behavior signature detected
    pub behavior: BehaviorSignature,
    /// Semantic mismatch analysis
    pub mismatch: SemanticMismatch,
    /// Name proposals if mismatch detected
    pub proposals: Vec<NameProposal>,
    /// Impact of renaming this function
    pub impact_score: f64,
}

/// Simplified semantic name analyzer using rule-based analysis
pub struct SimpleNameAnalyzer {
    config: NamesConfig,
}

impl SimpleNameAnalyzer {
    /// Create new simple name analyzer
    pub fn new(config: NamesConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(NamesConfig::default())
    }

    /// Analyze files for naming issues
    pub async fn analyze_files(&self, file_paths: &[&Path]) -> Result<Vec<NamingAnalysisResult>> {
        if !self.config.enabled {
            return Ok(Vec::new());
        }

        info!("Running simplified naming analysis on {} files", file_paths.len());
        let mut results = Vec::new();

        for file_path in file_paths {
            match self.analyze_file(file_path).await {
                Ok(mut file_results) => results.append(&mut file_results),
                Err(e) => warn!("Naming analysis failed for {}: {}", file_path.display(), e),
            }
        }

        info!("Naming analysis found {} potential issues", results.len());
        Ok(results)
    }

    /// Analyze a single file for naming issues
    async fn analyze_file(&self, file_path: &Path) -> Result<Vec<NamingAnalysisResult>> {
        debug!("Analyzing naming for file: {}", file_path.display());

        let content = std::fs::read_to_string(file_path)
            .map_err(|e| ValknutError::io(format!("Failed to read file {}: {}", file_path.display(), e), e))?;

        // Extract functions from the file (simplified regex-based approach)
        let functions = self.extract_functions_simple(&content, file_path)?;
        let mut results = Vec::new();

        for func in functions {
            // Extract behavior signature
            let behavior = self.extract_behavior_signature(&func, &content);
            
            // Check for semantic mismatch
            let mismatch = self.check_semantic_mismatch(&func.name, &behavior);
            
            // Skip if mismatch score is below threshold
            if mismatch.mismatch_score < self.config.min_mismatch {
                continue;
            }

            // Generate name proposals
            let proposals = self.generate_name_proposals(&func.name, &behavior);
            
            // Calculate impact score (simplified)
            let impact_score = self.calculate_impact_score(&func, &content);
            
            // Skip if impact is below threshold
            if impact_score < self.config.min_impact as f64 {
                continue;
            }

            results.push(NamingAnalysisResult {
                function_id: format!("{}:{}", file_path.display(), func.line),
                current_name: func.name.clone(),
                file_path: file_path.to_string_lossy().to_string(),
                line_number: func.line,
                behavior,
                mismatch,
                proposals,
                impact_score,
            });
        }

        Ok(results)
    }

    /// Simple function extraction using pattern matching
    fn extract_functions_simple(&self, content: &str, file_path: &Path) -> Result<Vec<FunctionInfo>> {
        let mut functions = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let language = self.detect_language(file_path);

        for (line_num, line) in lines.iter().enumerate() {
            if let Some(func_info) = self.extract_function_from_line(line, line_num + 1, &language) {
                functions.push(func_info);
            }
        }

        Ok(functions)
    }

    /// Extract function information from a single line
    fn extract_function_from_line(&self, line: &str, line_num: usize, language: &str) -> Option<FunctionInfo> {
        let trimmed = line.trim();
        
        match language {
            "python" => {
                if let Some(captures) = regex::Regex::new(r"def\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").ok()?.captures(trimmed) {
                    Some(FunctionInfo {
                        name: captures.get(1)?.as_str().to_string(),
                        line: line_num,
                        is_async: trimmed.contains("async def"),
                        visibility: if trimmed.starts_with(' ') { "private" } else { "public" }.to_string(),
                    })
                } else {
                    None
                }
            }
            "javascript" | "typescript" => {
                if let Some(captures) = regex::Regex::new(r"function\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").ok()?.captures(trimmed) {
                    Some(FunctionInfo {
                        name: captures.get(1)?.as_str().to_string(),
                        line: line_num,
                        is_async: trimmed.contains("async "),
                        visibility: "public".to_string(), // Simplified
                    })
                } else if let Some(captures) = regex::Regex::new(r"const\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(?:async\s+)?\(").ok()?.captures(trimmed) {
                    Some(FunctionInfo {
                        name: captures.get(1)?.as_str().to_string(),
                        line: line_num,
                        is_async: trimmed.contains("async"),
                        visibility: "public".to_string(),
                    })
                } else {
                    None
                }
            }
            "rust" => {
                if let Some(captures) = regex::Regex::new(r"fn\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(").ok()?.captures(trimmed) {
                    Some(FunctionInfo {
                        name: captures.get(1)?.as_str().to_string(),
                        line: line_num,
                        is_async: trimmed.contains("async fn"),
                        visibility: if trimmed.starts_with("pub") { "public" } else { "private" }.to_string(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Extract behavior signature from function (simplified heuristics)
    fn extract_behavior_signature(&self, func: &FunctionInfo, content: &str) -> BehaviorSignature {
        let name_lower = func.name.to_lowercase();
        
        // Analyze side effects based on naming patterns and content
        let side_effects = SideEffects {
            has_database_ops: name_lower.contains("db") || name_lower.contains("sql") || 
                             name_lower.contains("query") || content.contains("SELECT") || content.contains("INSERT"),
            has_file_ops: name_lower.contains("file") || name_lower.contains("read") || 
                         name_lower.contains("write") || content.contains("open(") || content.contains("File"),
            has_network_ops: name_lower.contains("fetch") || name_lower.contains("request") || 
                            name_lower.contains("http") || content.contains("requests.") || content.contains("fetch("),
            has_mutations: name_lower.starts_with("set_") || name_lower.starts_with("update_") || 
                          name_lower.starts_with("create_") || name_lower.starts_with("delete_"),
        };

        // Determine execution pattern
        let execution_pattern = if func.is_async {
            ExecutionPattern::Asynchronous
        } else {
            ExecutionPattern::Synchronous
        };

        // Analyze return type based on naming patterns
        let return_type = ReturnTypeInfo {
            optional: name_lower.starts_with("find_") || name_lower.starts_with("try_") || name_lower.contains("maybe"),
            collection: name_lower.contains("list") || name_lower.ends_with("s") || name_lower.contains("all"),
            type_category: if name_lower.contains("list") || name_lower.ends_with("s") {
                TypeCategory::Collection
            } else if name_lower.starts_with("is_") || name_lower.starts_with("has_") {
                TypeCategory::Scalar
            } else {
                TypeCategory::Object
            },
        };

        // Calculate confidence based on available information
        let confidence = if side_effects.has_database_ops || side_effects.has_file_ops || side_effects.has_network_ops {
            0.8 // High confidence for I/O operations
        } else {
            0.6 // Medium confidence for pure naming analysis
        };

        BehaviorSignature {
            side_effects,
            return_type,
            execution_pattern,
            confidence,
        }
    }

    /// Check for semantic mismatch using rule-based analysis
    fn check_semantic_mismatch(&self, name: &str, behavior: &BehaviorSignature) -> SemanticMismatch {
        let mut mismatch_types = Vec::new();
        let name_lower = name.to_lowercase();

        // Effect mismatch detection
        if name_lower.starts_with("get_") || name_lower.starts_with("is_") || name_lower.starts_with("has_") {
            if behavior.side_effects.has_mutations {
                mismatch_types.push(MismatchType::EffectMismatch {
                    expected: "read-only operation".to_string(),
                    actual: "modifies state".to_string(),
                });
            }
        }

        // Cardinality mismatch
        if behavior.return_type.collection && !name_lower.contains("list") && 
           !name_lower.ends_with("s") && !name_lower.contains("all") {
            mismatch_types.push(MismatchType::CardinalityMismatch {
                expected: "single item".to_string(),
                actual: "collection".to_string(),
            });
        }

        // Optionality mismatch
        if (name_lower.starts_with("find_") || name_lower.starts_with("try_")) && !behavior.return_type.optional {
            mismatch_types.push(MismatchType::OptionalityMismatch {
                expected: "optional return".to_string(),
                actual: "guaranteed return".to_string(),
            });
        }

        // Async mismatch
        match behavior.execution_pattern {
            ExecutionPattern::Asynchronous => {
                if !name_lower.contains("async") {
                    mismatch_types.push(MismatchType::AsyncMismatch {
                        expected: "synchronous".to_string(),
                        actual: "asynchronous".to_string(),
                    });
                }
            },
            ExecutionPattern::Synchronous => {
                if name_lower.contains("async") {
                    mismatch_types.push(MismatchType::AsyncMismatch {
                        expected: "asynchronous".to_string(),
                        actual: "synchronous".to_string(),
                    });
                }
            },
            ExecutionPattern::Ambiguous => {} // No mismatch for ambiguous
        }

        // Calculate rule-based similarity (inverted - lower means more mismatched)
        let similarity_score = 1.0 - (mismatch_types.len() as f64 * 0.2).min(1.0);
        
        // Calculate overall mismatch score
        let mismatch_score = 1.0 - similarity_score;
        
        // Calculate confidence based on behavior confidence and name clarity
        let confidence = behavior.confidence * 0.8; // Rule-based is less confident than embedding-based

        SemanticMismatch {
            similarity_score,
            mismatch_types,
            mismatch_score,
            confidence,
        }
    }

    /// Generate name proposals based on behavior
    fn generate_name_proposals(&self, current_name: &str, behavior: &BehaviorSignature) -> Vec<NameProposal> {
        let mut proposals = Vec::new();

        // Generate verb based on behavior
        let verb = if behavior.side_effects.has_database_ops {
            if behavior.side_effects.has_mutations {
                "update"
            } else {
                "get"
            }
        } else if behavior.side_effects.has_file_ops {
            if behavior.side_effects.has_mutations {
                "save"
            } else {
                "load"
            }
        } else if behavior.side_effects.has_network_ops {
            "fetch"
        } else if behavior.return_type.collection {
            "list"
        } else if behavior.return_type.optional {
            "find"
        } else {
            "get"
        };

        // Extract noun from current name
        let parts: Vec<&str> = current_name.split('_').collect();
        let noun = if parts.len() > 1 { parts[1] } else { "data" };

        // Generate proposals
        let base_name = format!("{}_{}", verb, noun);
        proposals.push(NameProposal {
            name: base_name.clone(),
            rationale: format!("Based on {} behavior pattern", verb),
            confidence: 0.8,
        });

        // Add async suffix if needed
        if matches!(behavior.execution_pattern, ExecutionPattern::Asynchronous) {
            proposals.push(NameProposal {
                name: format!("{}_async", base_name),
                rationale: "Added async suffix for asynchronous operation".to_string(),
                confidence: 0.7,
            });
        }

        // Add collection suffix if needed
        if behavior.return_type.collection && !base_name.ends_with("s") {
            proposals.push(NameProposal {
                name: format!("{}s", base_name.trim_end_matches("_data")).to_string(),
                rationale: "Pluralized for collection return type".to_string(),
                confidence: 0.6,
            });
        }

        proposals
    }

    /// Calculate impact score for function renaming
    fn calculate_impact_score(&self, func: &FunctionInfo, content: &str) -> f64 {
        // Simple heuristic: count occurrences of function name in file
        let references = content.matches(&func.name).count();
        
        // Public functions have higher impact
        let visibility_multiplier = if func.visibility == "public" { 2.0 } else { 1.0 };
        
        (references as f64 * visibility_multiplier).max(1.0)
    }

    /// Detect programming language from file path
    fn detect_language(&self, file_path: &Path) -> String {
        match file_path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "rs" => "rust",
            "go" => "go",
            _ => "unknown",
        }.to_string()
    }
}

/// Simple function information
#[derive(Debug, Clone)]
struct FunctionInfo {
    name: String,
    line: usize,
    is_async: bool,
    visibility: String,
}