//! Configuration structs, data types, and core types for structure analysis

use petgraph::{Directed, Graph, Undirected};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// Code file extensions recognized for structure analysis
pub const CODE_EXTENSIONS: &[&str] = &[
    "py", "pyi", "js", "mjs", "ts", "jsx", "tsx", "rs", "go", "java", "cpp", "c", "h", "hpp",
];

/// Check if an extension is a recognized code file extension
#[inline]
pub fn is_code_extension(extension: &str) -> bool {
    CODE_EXTENSIONS.contains(&extension)
}

/// Directories to skip during structure analysis
pub const SKIP_DIRECTORIES: &[&str] = &[
    "node_modules", "target", ".git", "__pycache__", "dist", "build", ".next", "vendor", "venv",
];

/// Check if a path should be skipped during analysis.
///
/// Returns true if the path contains any of the skip directory names.
#[inline]
pub fn should_skip_directory(path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();
    SKIP_DIRECTORIES.iter().any(|d| path_str.contains(d))
}

/// Configuration for structure analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureConfig {
    /// Enable branch reorganization packs
    pub enable_branch_packs: bool,
    /// Enable file split packs
    pub enable_file_split_packs: bool,
    /// Maximum number of top packs to return
    pub top_packs: usize,
    /// File system directory settings
    pub fsdir: FsDirectoryConfig,
    /// File system file settings
    pub fsfile: FsFileConfig,
    /// Graph partitioning settings
    pub partitioning: PartitioningConfig,
    /// Entity health scoring settings
    pub entity_health: EntityHealthConfig,
}

/// Feature toggles for structure analysis outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureToggles {
    /// Enable branch reorganization packs
    pub enable_branch_packs: bool,
    /// Enable file split packs
    pub enable_file_split_packs: bool,
    /// Maximum number of top packs to return
    pub top_packs: usize,
}

/// Configuration for directory-level structure analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsDirectoryConfig {
    /// Maximum files per directory before pressure
    pub max_files_per_dir: usize,
    /// Maximum subdirectories per directory before pressure
    pub max_subdirs_per_dir: usize,
    /// Maximum lines of code per directory before pressure
    pub max_dir_loc: usize,
    /// Minimum imbalance gain required for branch recommendation
    pub min_branch_recommendation_gain: f64,
    /// Minimum files required before considering directory split
    pub min_files_for_split: usize,
    /// Target lines of code per subdirectory when partitioning
    pub target_loc_per_subdir: usize,
    /// Optimal number of files per directory (mean of distribution)
    pub optimal_files: usize,
    /// Standard deviation for file count distribution scoring
    pub optimal_files_stddev: f64,
    /// Optimal number of subdirectories per directory (mean of distribution)
    pub optimal_subdirs: usize,
    /// Standard deviation for subdirectory count distribution scoring
    pub optimal_subdirs_stddev: f64,
}

/// Configuration for file-level structure analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsFileConfig {
    /// Lines of code threshold for huge files
    pub huge_loc: usize,
    /// Byte size threshold for huge files
    pub huge_bytes: usize,
    /// Minimum lines of code before considering file split
    pub min_split_loc: usize,
    /// Minimum entities per file split
    pub min_entities_per_split: usize,
    /// Optimal file size in AST nodes (mode of lognormal distribution).
    /// Default 2000 corresponds to ~200 LOC at ~10 nodes/line.
    pub optimal_ast_nodes: usize,
    /// AST node count at 95th percentile of the lognormal distribution.
    /// Default 6000 corresponds to ~600 LOC. Used to derive distribution shape.
    pub ast_nodes_95th_percentile: usize,
}

/// Configuration for entity health scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityHealthConfig {
    /// Function/method size scoring parameters
    pub function_size: EntitySizeParams,
    /// Class/struct size scoring parameters
    pub class_size: EntitySizeParams,
    /// File size scoring parameters
    pub file_size: EntitySizeParams,
}

/// Parameters for entity size health scoring using lognormal distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySizeParams {
    /// Optimal size in AST nodes (mode of lognormal distribution)
    pub optimal: usize,
    /// Size at 95th percentile of the lognormal distribution
    pub percentile_95: usize,
    /// Logistic shaping: percentile where penalty ramp centers (0.0-1.0)
    pub penalty_center: f64,
    /// Logistic shaping: steepness of the penalty ramp (smaller = steeper)
    pub penalty_steepness: f64,
}

/// Default implementation for [`EntityHealthConfig`].
impl Default for EntityHealthConfig {
    /// Returns entity health configuration with balanced defaults.
    fn default() -> Self {
        Self {
            function_size: EntitySizeParams {
                // ~20 LOC optimal (~200 AST nodes), ~80 LOC at P95 (~800 AST nodes)
                optimal: 200,
                percentile_95: 800,
                penalty_center: 0.90,
                penalty_steepness: 0.05,
            },
            class_size: EntitySizeParams {
                // ~50 LOC optimal (~500 AST nodes), ~300 LOC at P95 (~3000 AST nodes)
                optimal: 500,
                percentile_95: 3000,
                penalty_center: 0.90,
                penalty_steepness: 0.05,
            },
            file_size: EntitySizeParams {
                // ~200 LOC optimal (~2000 AST nodes), ~600 LOC at P95 (~6000 AST nodes)
                optimal: 2000,
                percentile_95: 6000,
                penalty_center: 0.90,
                penalty_steepness: 0.05,
            },
        }
    }
}

/// Configuration for directory partitioning algorithms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitioningConfig {
    /// Balance tolerance for partitioning (0.25 = ±25%)
    pub balance_tolerance: f64,
    /// Maximum number of clusters per partition
    pub max_clusters: usize,
    /// Minimum number of clusters per partition
    pub min_clusters: usize,
    /// Fallback names for clusters when automatic naming fails
    pub naming_fallbacks: Vec<String>,
}

/// Default implementation for [`StructureConfig`].
impl Default for StructureConfig {
    /// Returns structure analysis configuration with balanced defaults.
    fn default() -> Self {
        Self {
            enable_branch_packs: true,
            enable_file_split_packs: true,
            top_packs: 20,
            fsdir: FsDirectoryConfig {
                max_files_per_dir: 25,
                max_subdirs_per_dir: 10,
                max_dir_loc: 2000,
                min_branch_recommendation_gain: 0.15,
                min_files_for_split: 5,
                target_loc_per_subdir: 1000,
                optimal_files: 7,
                optimal_files_stddev: 2.0,
                optimal_subdirs: 3,
                optimal_subdirs_stddev: 1.5,
            },
            fsfile: FsFileConfig {
                huge_loc: 800,
                huge_bytes: 128_000,
                min_split_loc: 200,
                min_entities_per_split: 3,
                optimal_ast_nodes: 2000,
                ast_nodes_95th_percentile: 6000,
            },
            partitioning: PartitioningConfig {
                balance_tolerance: 0.25,
                max_clusters: 4,
                min_clusters: 2,
                naming_fallbacks: vec![
                    "core".to_string(),
                    "io".to_string(),
                    "api".to_string(),
                    "util".to_string(),
                ],
            },
            entity_health: EntityHealthConfig::default(),
        }
    }
}

/// Directory metrics for imbalance calculation
#[derive(Debug, Clone, Serialize)]
pub struct DirectoryMetrics {
    /// Number of files in directory
    pub files: usize,
    /// Number of subdirectories
    pub subdirs: usize,
    /// Total lines of code
    pub loc: usize,
    /// Gini coefficient of LOC distribution
    pub gini: f64,
    /// Entropy of LOC distribution
    pub entropy: f64,
    /// File pressure (files / max_files_per_dir)
    pub file_pressure: f64,
    /// Branch pressure (subdirs / max_subdirs_per_dir)
    pub branch_pressure: f64,
    /// Size pressure (loc / max_dir_loc)
    pub size_pressure: f64,
    /// Dispersion metric combining gini and entropy
    pub dispersion: f64,
    /// Distribution-based score for file count (1.0 = optimal, decreases away from optimal)
    pub file_count_score: f64,
    /// Distribution-based score for subdirectory count (1.0 = optimal, decreases away from optimal)
    pub subdir_count_score: f64,
    /// Overall imbalance score
    pub imbalance: f64,
}

/// File-level metrics for size scoring
#[derive(Debug, Clone, Serialize)]
pub struct FileMetrics {
    /// File path
    pub path: PathBuf,
    /// Number of AST nodes (named nodes only, excludes punctuation/whitespace)
    pub ast_nodes: usize,
    /// Lines of code (for reference, not used in scoring)
    pub loc: usize,
    /// Lognormal distribution-based score for file size (1.0 = optimal, decreases away from optimal)
    pub size_score: f64,
    /// Entity health summary for functions/classes in this file
    pub entity_health: Option<FileEntityHealth>,
}

/// Aggregated entity health metrics for a file
#[derive(Debug, Clone, Serialize)]
pub struct FileEntityHealth {
    /// Number of entities analyzed
    pub entity_count: usize,
    /// Total AST nodes across all entities
    pub total_ast_nodes: usize,
    /// AST-weighted average health score (0.0-1.0)
    pub health: f64,
    /// Minimum entity health (worst entity)
    pub min_health: f64,
}

/// Branch reorganization pack recommendation
#[derive(Debug, Clone, Serialize)]
pub struct BranchReorgPack {
    /// Type identifier
    pub kind: String,
    /// Directory path
    pub dir: PathBuf,
    /// Current directory state
    pub current: DirectoryMetrics,
    /// Proposed partitions
    pub proposal: Vec<DirectoryPartition>,
    /// File move operations
    pub file_moves: Vec<FileMove>,
    /// Expected gains from reorganization
    pub gain: ReorganizationGain,
    /// Estimated effort for reorganization
    pub effort: ReorganizationEffort,
    /// Rules and constraints
    pub rules: Vec<String>,
}

/// Proposed directory partition
#[derive(Debug, Clone, Serialize)]
pub struct DirectoryPartition {
    /// Suggested partition name
    pub name: String,
    /// Files to move to this partition
    pub files: Vec<PathBuf>,
    /// Total lines of code in partition
    pub loc: usize,
}

/// Expected gains from reorganization
#[derive(Debug, Clone, Serialize)]
pub struct ReorganizationGain {
    /// Change in imbalance score (positive = improvement)
    pub imbalance_delta: f64,
    /// Number of cross-cluster edges reduced
    pub cross_edges_reduced: usize,
}

/// Effort estimation for reorganization
#[derive(Debug, Clone, Serialize)]
pub struct ReorganizationEffort {
    /// Number of files that need to be moved
    pub files_moved: usize,
    /// Estimated number of import statement updates
    pub import_updates_est: usize,
}

/// File move operation
#[derive(Debug, Clone, Serialize)]
pub struct FileMove {
    /// Source file path
    pub from: PathBuf,
    /// Destination file path
    pub to: PathBuf,
}

/// File split pack recommendation
#[derive(Debug, Clone, Serialize)]
pub struct FileSplitPack {
    /// Type identifier
    pub kind: String,
    /// File path to split
    pub file: PathBuf,
    /// Reasons for splitting
    pub reasons: Vec<String>,
    /// Suggested split files
    pub suggested_splits: Vec<SuggestedSplit>,
    /// Value metrics
    pub value: SplitValue,
    /// Effort estimation
    pub effort: SplitEffort,
}

/// Suggested file split
#[derive(Debug, Clone, Serialize)]
pub struct SuggestedSplit {
    /// Name of the split file
    pub name: String,
    /// Entities (functions, classes) to move
    pub entities: Vec<String>,
    /// Lines of code in split
    pub loc: usize,
}

/// Value metrics for file splitting
#[derive(Debug, Clone, Serialize)]
pub struct SplitValue {
    /// Overall value score
    pub score: f64,
}

/// Effort estimation for file splitting
#[derive(Debug, Clone, Serialize)]
pub struct SplitEffort {
    /// Number of exports that need updating
    pub exports: usize,
    /// Number of external importers affected
    pub external_importers: usize,
}

/// Internal dependency graph for partitioning
pub type DependencyGraph = Graph<FileNode, DependencyEdge, Directed>;

/// File node in dependency graph
#[derive(Debug, Clone)]
pub struct FileNode {
    /// File path
    pub path: PathBuf,
    /// Lines of code
    pub loc: usize,
    /// File size in bytes
    pub size_bytes: usize,
}

/// Dependency edge in graph
#[derive(Debug, Clone)]
pub struct DependencyEdge {
    /// Weight (import count)
    pub weight: usize,
    /// Import type/relationship
    pub relationship_type: String,
}

/// Entity cohesion graph for file splitting
pub type CohesionGraph = Graph<EntityNode, CohesionEdge, Undirected>;

/// Entity node in cohesion graph
#[derive(Debug, Clone)]
pub struct EntityNode {
    /// Entity name (function, class, etc.)
    pub name: String,
    /// Entity type (function, class, etc.)
    pub entity_type: String,
    /// Lines of code for entity
    pub loc: usize,
    /// AST node count for entity (for health scoring)
    pub ast_nodes: usize,
    /// Referenced symbols/identifiers
    pub symbols: HashSet<String>,
}

/// Cohesion edge between entities
#[derive(Debug, Clone)]
pub struct CohesionEdge {
    /// Similarity weight (0.0 to 1.0)
    pub similarity: f64,
    /// Number of shared symbols
    pub shared_symbols: usize,
}

/// Import statement for dependency analysis
#[derive(Debug, Clone)]
pub struct ImportStatement {
    /// Module being imported
    pub module: String,
    /// Specific imports (None for star imports)
    pub imports: Option<Vec<String>>,
    /// Import type (default, named, star, etc.)
    pub import_type: String,
    /// Line number in file
    pub line_number: usize,
}
