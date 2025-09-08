//! Configuration structs, data types, and core types for semantic naming analysis.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

/// Configuration for semantic naming analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamesConfig {
    /// Enable semantic naming analysis
    pub enabled: bool,
    /// Embedding model to use (Qwen3-Embedding-0.6B-GGUF)
    pub embedding_model: String,
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
    /// I/O library patterns per language for effect detection
    pub io_libs: HashMap<String, Vec<String>>,
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

        let mut io_libs = HashMap::new();
        io_libs.insert("python".to_string(), vec![
            "requests".to_string(), "aiohttp".to_string(), "sqlalchemy".to_string(),
            "boto3".to_string(), "os".to_string(), "pathlib".to_string(),
            "json".to_string(), "sqlite3".to_string(), "psycopg2".to_string(),
        ]);
        io_libs.insert("typescript".to_string(), vec![
            "node:fs".to_string(), "fs".to_string(), "axios".to_string(),
            "fetch".to_string(), "pg".to_string(), "mongodb".to_string(),
            "sqlite3".to_string(), "redis".to_string(),
        ]);
        io_libs.insert("rust".to_string(), vec![
            "reqwest".to_string(), "tokio::fs".to_string(), "sqlx".to_string(),
            "rusqlite".to_string(), "redis".to_string(), "mongodb".to_string(),
        ]);

        Self {
            enabled: true,
            embedding_model: "Qwen/Qwen3-Embedding-0.6B-GGUF".to_string(),
            min_mismatch: 0.65,
            min_impact: 3,
            protect_public_api: true,
            abbrev_map,
            allowed_abbrevs: vec![
                "id".to_string(), "url".to_string(), "db".to_string(),
                "io".to_string(), "api".to_string(), "ui".to_string(),
                "os".to_string(), "fs".to_string(),
            ],
            io_libs,
        }
    }
}

/// Behavior signature extracted from static analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSignature {
    /// Side effects detected (I/O, database, network, file system)
    pub side_effects: SideEffects,
    /// Mutation patterns (modifies parameters, global state)
    pub mutations: MutationPattern,
    /// Async/synchronous execution pattern
    pub execution_pattern: ExecutionPattern,
    /// Return type characteristics
    pub return_type: ReturnTypeInfo,
    /// Resource handling (opens files, connections, etc.)
    pub resource_handling: ResourceHandling,
    /// Confidence in behavior inference (0.0-1.0)
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SideEffects {
    /// HTTP requests/API calls
    pub http_operations: bool,
    /// Database read/write operations
    pub database_operations: DatabaseOperations,
    /// File system operations
    pub file_operations: FileOperations,
    /// Network operations beyond HTTP
    pub network_operations: bool,
    /// Console/logging output
    pub console_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseOperations {
    pub reads: bool,
    pub writes: bool,
    pub deletes: bool,
    pub creates: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperations {
    pub reads: bool,
    pub writes: bool,
    pub creates: bool,
    pub deletes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MutationPattern {
    /// Does not mutate any state
    Pure,
    /// Modifies function parameters
    ParameterMutation,
    /// Modifies global/class state
    GlobalMutation,
    /// Both parameter and global mutations
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionPattern {
    /// Synchronous execution
    Synchronous,
    /// Returns Promise/Future
    Asynchronous,
    /// Can be either (overloaded)
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnTypeInfo {
    /// Primary return type (User, Config, String, etc.)
    pub primary_type: Option<String>,
    /// Whether return can be null/None/undefined
    pub optional: bool,
    /// Whether returns a collection/iterator
    pub collection: bool,
    /// Iterator/stream vs materialized collection
    pub lazy_evaluation: bool,
    /// Scalar, object, or complex type
    pub type_category: TypeCategory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TypeCategory {
    Scalar,
    Object,
    Collection,
    Stream,
    Resource,
    Unit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceHandling {
    /// Opens files, connections, etc.
    pub acquires_resources: bool,
    /// Explicitly releases resources
    pub releases_resources: bool,
    /// Returns resource handles
    pub returns_handles: bool,
}

/// Semantic mismatch between function name and behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMismatch {
    /// Cosine similarity between name and behavior (0.0-1.0)
    pub cosine_similarity: f64,
    /// Specific mismatch types detected
    pub mismatch_types: Vec<MismatchType>,
    /// Overall mismatch score (higher = more mismatched)
    pub mismatch_score: f64,
    /// Confidence in the mismatch detection
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MismatchType {
    /// Name implies no side effects but function has them
    EffectMismatch { expected: String, actual: String },
    /// Name implies different cardinality (singular vs plural)
    CardinalityMismatch { expected: String, actual: String },
    /// Name implies optional return but function guarantees return
    OptionalityMismatch { expected: String, actual: String },
    /// Name implies async but function is sync or vice versa
    AsyncMismatch { expected: String, actual: String },
    /// Name implies different operation type
    OperationMismatch { expected: String, actual: String },
}

/// Deterministic name proposal for a function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameProposal {
    /// Proposed function name
    pub name: String,
    /// Rationale for this name choice
    pub rationale: String,
    /// Confidence in this proposal (0.0-1.0)
    pub confidence: f64,
    /// Components used in name construction
    pub components: NameComponents,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NameComponents {
    /// Primary verb (get, create, update, etc.)
    pub verb: String,
    /// Primary noun (user, config, etc.)
    pub noun: String,
    /// Additional qualifiers (by_id, with_timeout, etc.)
    pub qualifiers: Vec<String>,
}

/// Rename recommendation pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenamePack {
    /// Function being analyzed
    pub function_id: String,
    /// Current function name
    pub current_name: String,
    /// File path where function is defined
    pub file_path: String,
    /// Line number of function definition
    pub line_number: usize,
    /// Top name proposals (ranked by confidence)
    pub proposals: Vec<NameProposal>,
    /// Impact analysis (callsites that would be affected)
    pub impact: ImpactAnalysis,
    /// Mismatch analysis that triggered this pack
    pub mismatch: SemanticMismatch,
    /// Priority score for ranking multiple packs
    pub priority: f64,
}

/// Contract mismatch recommendation pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractMismatchPack {
    /// Function being analyzed
    pub function_id: String,
    /// Current function name
    pub current_name: String,
    /// File path where function is defined
    pub file_path: String,
    /// Line number of function definition
    pub line_number: usize,
    /// Contract expectation vs reality
    pub contract_issues: Vec<ContractIssue>,
    /// Suggested solutions (rename or contract change)
    pub solutions: Vec<Solution>,
    /// Impact analysis
    pub impact: ImpactAnalysis,
    /// Priority score for ranking
    pub priority: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractIssue {
    /// Description of the contract violation
    pub description: String,
    /// What the name implies
    pub name_implies: String,
    /// What the function actually does
    pub actual_behavior: String,
    /// Severity of the mismatch
    pub severity: ContractSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContractSeverity {
    Low,    // Minor naming inconsistency
    Medium, // Potential confusion
    High,   // Likely to cause bugs
    Critical, // Almost certainly will cause bugs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Solution {
    /// Rename the function to match behavior
    Rename { to_name: String, rationale: String },
    /// Change function contract to match name
    ContractChange { description: String, effort: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    /// Number of external references (callsites)
    pub external_refs: usize,
    /// Number of files that would need updates
    pub affected_files: usize,
    /// Whether this affects public API
    pub public_api: bool,
    /// Estimated effort to make the change (1-10 scale)
    pub effort_estimate: u32,
    /// Files that would be affected by the change
    pub affected_locations: Vec<String>,
}

/// Project lexicon for consistency analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLexicon {
    /// Common nouns found in types/schemas/directories
    pub domain_nouns: HashMap<String, DomainNoun>,
    /// Verb patterns used across the project
    pub verb_patterns: HashMap<String, VerbUsage>,
    /// Synonym clusters (user/member/account)
    pub synonym_clusters: Vec<SynonymCluster>,
    /// Naming conventions per language
    pub naming_conventions: HashMap<String, NamingConvention>,
}

impl ProjectLexicon {
    pub fn new() -> Self {
        Self {
            domain_nouns: HashMap::new(),
            verb_patterns: HashMap::new(),
            synonym_clusters: Vec::new(),
            naming_conventions: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainNoun {
    /// The canonical noun form
    pub canonical: String,
    /// Alternative forms seen in codebase
    pub variants: HashSet<String>,
    /// Contexts where this noun appears
    pub contexts: Vec<String>,
    /// Frequency of usage
    pub frequency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbUsage {
    /// The verb
    pub verb: String,
    /// Associated effects/operations
    pub typical_effects: HashSet<String>,
    /// Usage frequency
    pub frequency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynonymCluster {
    /// Related terms that should be consistent
    pub terms: HashSet<String>,
    /// Suggested canonical term
    pub canonical: String,
    /// Import graph overlap indicating relation
    pub overlap_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamingConvention {
    /// snake_case, camelCase, PascalCase
    pub case_style: String,
    /// Common prefixes/suffixes
    pub common_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistencyIssue {
    pub description: String,
    pub synonyms: Vec<String>,
    pub suggested_canonical: String,
}

/// Placeholder types for the interface
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub id: String,
    pub name: String,
    pub file_path: String,
    pub line_number: usize,
    pub visibility: String,
    pub parameters: Vec<ParameterInfo>,
    pub return_type: Option<String>,
    pub body_ast: Option<crate::lang::common::AstNode>,
    pub call_sites: Vec<CallSite>,
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CallSite {
    pub file_path: String,
    pub line_number: usize,
}

#[derive(Debug)]
pub enum AnalysisResult {
    RenamePack(RenamePack),
    ContractMismatchPack(ContractMismatchPack),
}

#[derive(Debug)]
pub struct AnalysisResults {
    pub rename_packs: Vec<RenamePack>,
    pub contract_mismatch_packs: Vec<ContractMismatchPack>,
    pub lexicon_consistency: Vec<ConsistencyIssue>,
}