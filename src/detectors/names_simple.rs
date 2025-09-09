//! Simplified semantic naming analyzer using rule-based analysis.
//!
//! This module implements a deterministic semantic naming analysis system that:
//! - Extracts behavior signatures from code using AST analysis
//! - Uses rule-based semantic matching instead of embeddings
//! - Applies deterministic naming rules based on observed effects
//! - Generates rename recommendations and contract mismatch analysis
//! - Maintains project consistency through lexicon building

use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;

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

        let content = FileReader::read_to_string(file_path)?;

        // Extract functions from the file (simplified regex-based approach)
        let functions = self.extract_functions_simple(&content, file_path)?;
        println!("Extracted functions: {:?}", functions);
        let mut results = Vec::new();

        for func in functions {
            // Extract behavior signature
            let behavior = self.extract_behavior_signature(&func, &content);
            println!("Behavior for {}: {:?}", func.name, behavior);
            
            // Check for semantic mismatch
            let mismatch = self.check_semantic_mismatch(&func.name, &behavior);
            println!("Mismatch for {}: score={}, threshold={}", func.name, mismatch.mismatch_score, self.config.min_mismatch);
            
            // Skip if mismatch score is below threshold
            if mismatch.mismatch_score < self.config.min_mismatch {
                println!("Skipping {} due to low mismatch score", func.name);
                continue;
            }

            // Generate name proposals
            let proposals = self.generate_name_proposals(&func.name, &behavior);
            
            // Calculate impact score (simplified)
            let impact_score = self.calculate_impact_score(&func, &content);
            println!("Impact score for {}: {}, threshold: {}", func.name, impact_score, self.config.min_impact);
            
            // Skip if impact is below threshold
            if impact_score < self.config.min_impact as f64 {
                println!("Skipping {} due to low impact score", func.name);
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
                          name_lower.starts_with("create_") || name_lower.starts_with("delete_") ||
                          content.contains(".update(") || content.contains(".save(") || 
                          content.contains(".insert(") || content.contains(".delete(") ||
                          content.contains(".modify(") || content.contains(".append(") ||
                          content.contains(".push(") || content.contains(".pop(") ||
                          content.contains("=") && !content.contains("==") && !content.contains("!="),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use std::fs;

    #[test]
    fn test_names_config_default() {
        let config = NamesConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_mismatch, 0.65);
        assert_eq!(config.min_impact, 3);
        assert!(config.protect_public_api);
        assert!(config.abbrev_map.contains_key("usr"));
        assert!(config.allowed_abbrevs.contains(&"id".to_string()));
    }

    #[test]
    fn test_simple_name_analyzer_creation() {
        let analyzer = SimpleNameAnalyzer::default();
        assert!(analyzer.config.enabled);
        
        let custom_config = NamesConfig {
            enabled: false,
            ..Default::default()
        };
        let analyzer = SimpleNameAnalyzer::new(custom_config);
        assert!(!analyzer.config.enabled);
    }

    #[tokio::test]
    async fn test_analyze_files_disabled() {
        let config = NamesConfig {
            enabled: false,
            ..Default::default()
        };
        let analyzer = SimpleNameAnalyzer::new(config);
        
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        fs::write(&file_path, "def test_function():\n    pass").unwrap();
        
        let paths = vec![file_path.as_path()];
        let results = analyzer.analyze_files(&paths).await.unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_detect_language() {
        let analyzer = SimpleNameAnalyzer::default();
        
        assert_eq!(analyzer.detect_language(Path::new("test.py")), "python");
        assert_eq!(analyzer.detect_language(Path::new("test.js")), "javascript");
        assert_eq!(analyzer.detect_language(Path::new("test.ts")), "typescript");
        assert_eq!(analyzer.detect_language(Path::new("test.rs")), "rust");
        assert_eq!(analyzer.detect_language(Path::new("test.go")), "go");
        assert_eq!(analyzer.detect_language(Path::new("test.txt")), "unknown");
    }

    #[test]
    fn test_extract_function_from_line_python() {
        let analyzer = SimpleNameAnalyzer::default();
        
        // Test Python function
        let func = analyzer.extract_function_from_line("def test_func():", 1, "python");
        assert!(func.is_some());
        let func = func.unwrap();
        assert_eq!(func.name, "test_func");
        assert_eq!(func.line, 1);
        assert!(!func.is_async);
        
        // Test async Python function
        let func = analyzer.extract_function_from_line("async def async_func():", 2, "python");
        assert!(func.is_some());
        let func = func.unwrap();
        assert_eq!(func.name, "async_func");
        assert!(func.is_async);
    }

    #[test]
    fn test_extract_function_from_line_rust() {
        let analyzer = SimpleNameAnalyzer::default();
        
        // Test Rust function
        let func = analyzer.extract_function_from_line("fn test_func() {", 1, "rust");
        assert!(func.is_some());
        let func = func.unwrap();
        assert_eq!(func.name, "test_func");
        assert_eq!(func.visibility, "private");
        
        // Test public Rust function
        let func = analyzer.extract_function_from_line("pub fn public_func() {", 2, "rust");
        assert!(func.is_some());
        let func = func.unwrap();
        assert_eq!(func.name, "public_func");
        assert_eq!(func.visibility, "public");
        
        // Test async Rust function
        let func = analyzer.extract_function_from_line("pub async fn async_func() {", 3, "rust");
        assert!(func.is_some());
        let func = func.unwrap();
        assert_eq!(func.name, "async_func");
        assert!(func.is_async);
    }

    #[test]
    fn test_extract_behavior_signature() {
        let analyzer = SimpleNameAnalyzer::default();
        
        let func = FunctionInfo {
            name: "get_user_data".to_string(),
            line: 1,
            is_async: false,
            visibility: "public".to_string(),
        };
        
        let content = "SELECT * FROM users";
        let behavior = analyzer.extract_behavior_signature(&func, content);
        
        assert!(behavior.side_effects.has_database_ops);
        assert!(!behavior.side_effects.has_file_ops);
        assert!(!behavior.side_effects.has_network_ops);
        assert!(!behavior.side_effects.has_mutations);
        assert!(matches!(behavior.execution_pattern, ExecutionPattern::Synchronous));
        assert_eq!(behavior.confidence, 0.8);
    }

    #[test]
    fn test_check_semantic_mismatch() {
        let analyzer = SimpleNameAnalyzer::default();
        
        let behavior = BehaviorSignature {
            side_effects: SideEffects {
                has_database_ops: false,
                has_file_ops: false,
                has_network_ops: false,
                has_mutations: true,
            },
            return_type: ReturnTypeInfo {
                optional: false,
                collection: false,
                type_category: TypeCategory::Unit,
            },
            execution_pattern: ExecutionPattern::Synchronous,
            confidence: 0.8,
        };
        
        // Test effect mismatch - get_ function that mutates
        let mismatch = analyzer.check_semantic_mismatch("get_user", &behavior);
        assert!(!mismatch.mismatch_types.is_empty());
        assert!(mismatch.mismatch_types.iter().any(|m| matches!(m, MismatchType::EffectMismatch { .. })));
        assert!(mismatch.mismatch_score > 0.0);
    }

    #[test]
    fn test_generate_name_proposals() {
        let analyzer = SimpleNameAnalyzer::default();
        
        let behavior = BehaviorSignature {
            side_effects: SideEffects {
                has_database_ops: true,
                has_file_ops: false,
                has_network_ops: false,
                has_mutations: false,
            },
            return_type: ReturnTypeInfo {
                optional: false,
                collection: true,
                type_category: TypeCategory::Collection,
            },
            execution_pattern: ExecutionPattern::Asynchronous,
            confidence: 0.8,
        };
        
        let proposals = analyzer.generate_name_proposals("bad_name", &behavior);
        assert!(!proposals.is_empty());
        
        // Should suggest database-related verbs
        assert!(proposals.iter().any(|p| p.name.contains("get")));
    }

    #[test]
    fn test_calculate_impact_score() {
        let analyzer = SimpleNameAnalyzer::default();
        
        let func = FunctionInfo {
            name: "test_func".to_string(),
            line: 1,
            is_async: false,
            visibility: "public".to_string(),
        };
        
        let content = "test_func() + test_func() + other_func()";
        let impact = analyzer.calculate_impact_score(&func, content);
        
        // Should be 2 references * 2.0 (public multiplier) = 4.0
        assert_eq!(impact, 4.0);
        
        let private_func = FunctionInfo {
            name: "test_func".to_string(),
            line: 1,
            is_async: false,
            visibility: "private".to_string(),
        };
        
        let private_impact = analyzer.calculate_impact_score(&private_func, content);
        // Should be 2 references * 1.0 (private multiplier) = 2.0
        assert_eq!(private_impact, 2.0);
    }

    #[tokio::test]
    async fn test_analyze_file_integration() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.py");
        
        // Create a Python file with a problematic function name
        let content = r#"
def get_user_data():
    # This function actually modifies data
    user.update({"last_seen": now()})
    database.save(user)
    return user
"#;
        fs::write(&file_path, content).unwrap();
        
        let config = NamesConfig {
            enabled: true,
            min_mismatch: 0.1, // Lower threshold for test
            min_impact: 1, // Lower impact threshold for test
            ..Default::default()
        };
        let analyzer = SimpleNameAnalyzer::new(config);
        let results = analyzer.analyze_file(&file_path).await.unwrap();
        
        // Should detect the mismatch between "get_" and mutation behavior
        println!("Results found: {:?}", results);
        assert!(!results.is_empty());
        let result = &results[0];
        assert_eq!(result.current_name, "get_user_data");
        assert!(result.mismatch.mismatch_score >= analyzer.config.min_mismatch);
    }

    #[test]
    fn test_mismatch_type_variants() {
        // Test all MismatchType variants can be created
        let _effect = MismatchType::EffectMismatch {
            expected: "read".to_string(),
            actual: "write".to_string(),
        };
        
        let _cardinality = MismatchType::CardinalityMismatch {
            expected: "single".to_string(),
            actual: "collection".to_string(),
        };
        
        let _optionality = MismatchType::OptionalityMismatch {
            expected: "optional".to_string(),
            actual: "required".to_string(),
        };
        
        let _async_mismatch = MismatchType::AsyncMismatch {
            expected: "sync".to_string(),
            actual: "async".to_string(),
        };
        
        let _operation = MismatchType::OperationMismatch {
            expected: "read".to_string(),
            actual: "write".to_string(),
        };
    }

    #[test]
    fn test_type_category_variants() {
        use TypeCategory::*;
        
        // Test all variants
        let _scalar = Scalar;
        let _object = Object;
        let _collection = Collection;
        let _unit = Unit;
    }

    #[test]
    fn test_execution_pattern_variants() {
        use ExecutionPattern::*;
        
        // Test all variants
        let _sync = Synchronous;
        let _async = Asynchronous;
        let _ambiguous = Ambiguous;
    }
}