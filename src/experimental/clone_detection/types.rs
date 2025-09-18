//! Shared types for clone detection system

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Structural pattern in code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralPattern {
    pub signature: String,
    pub frequency: usize,
    pub complexity_score: f64,
}

/// PDG Motif representing a structural pattern
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PdgMotif {
    pub pattern: String,
    pub motif_type: MotifType,
    pub category: MotifCategory,
    pub weight: u32,
}

/// Type of PDG motif
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MotifType {
    Control,
    Data,
    Combined,
}

/// Category of motif for fine-grained analysis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MotifCategory {
    Loop,
    Conditional,
    Assignment,
    Call,
    Return,
    Declaration,
}

/// Basic block in control flow analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub id: String,
    pub statements: Vec<String>,
    pub successors: Vec<String>,
    pub predecessors: Vec<String>,
    pub dominance_level: u32,
    pub loop_depth: u32,
    pub control_dependencies: Vec<String>,
    pub data_dependencies: Vec<String>,
    pub region_id: Option<String>,
    pub is_loop_header: bool,
    pub is_loop_exit: bool,
    pub contains_call: bool,
    pub contains_return: bool,
    pub estimated_execution_frequency: f64,
}

/// Structural match information for region analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralMatchInfo {
    pub start_line: usize,
    pub end_line: usize,
    pub control_type: ControlType,
    pub nesting_level: usize,
    pub estimated_complexity: f64,
    pub contains_loops: bool,
    pub contains_calls: bool,
    pub variable_usage_pattern: HashMap<String, usize>,
}

/// Control flow type for structural matching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlType {
    Sequential,
    Conditional,
    Loop,
    Function,
    Exception,
}

/// Clone candidate with comprehensive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneCandidate {
    pub id: String,
    pub entities: Vec<String>,
    pub similarity_score: f64,
    pub structural_score: f64,
    pub lexical_score: f64,
    pub semantic_score: f64,
    pub size_normalized_score: f64,
    pub confidence: f64,
    pub clone_type: CloneType,
}

/// Filtered clone candidate with additional filtering metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredCloneCandidate {
    pub candidate: CloneCandidate,
    pub rejection_reason: Option<String>,
    pub passed_filters: Vec<String>,
    pub failed_filters: Vec<String>,
    pub filtering_stats: FilteringStatistics,
}

/// Detailed motif analysis results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotifAnalysisDetails {
    pub total_motifs_found: usize,
    pub unique_motifs: usize,
    pub most_common_motifs: Vec<(PdgMotif, usize)>,
    pub complexity_distribution: BTreeMap<String, f64>,
}

/// Noise analysis metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseMetrics {
    pub noise_ratio: f64,
    pub signal_strength: f64,
    pub boilerplate_percentage: f64,
    pub unique_content_ratio: f64,
    pub structural_diversity_score: f64,
    pub estimated_false_positive_rate: f64,
    pub confidence_interval: (f64, f64),
    pub sample_size: usize,
    pub analysis_timestamp: u64,
    pub convergence_iterations: usize,
}

impl Default for NoiseMetrics {
    fn default() -> Self {
        Self {
            noise_ratio: 0.0,
            signal_strength: 1.0,
            boilerplate_percentage: 0.0,
            unique_content_ratio: 1.0,
            structural_diversity_score: 0.5,
            estimated_false_positive_rate: 0.1,
            confidence_interval: (0.0, 1.0),
            sample_size: 0,
            analysis_timestamp: 0,
            convergence_iterations: 0,
        }
    }
}

/// Adaptive thresholds for dynamic filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveThresholds {
    pub similarity_threshold: f64,
    pub structural_threshold: f64,
    pub size_threshold: usize,
    pub complexity_threshold: f64,
    pub confidence_threshold: f64,
    pub noise_tolerance: f64,
    pub last_updated: u64,
    pub adaptation_rate: f64,
    pub stability_metric: f64,
    pub performance_history: Vec<f64>,
}

impl Default for AdaptiveThresholds {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.8,
            structural_threshold: 0.7,
            size_threshold: 10,
            complexity_threshold: 0.6,
            confidence_threshold: 0.8,
            noise_tolerance: 0.1,
            last_updated: 0,
            adaptation_rate: 0.1,
            stability_metric: 0.8,
            performance_history: Vec::new(),
        }
    }
}

/// Phase 2 filtering statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase2FilteringStats {
    pub total_candidates_input: usize,
    pub candidates_after_basic_blocks: usize,
    pub candidates_after_structural_gates: usize,
    pub candidates_after_external_calls: usize,
    pub candidates_after_io_penalty: usize,
    pub final_candidates_output: usize,
    pub filtering_time_ms: u64,
    pub average_structural_score: f64,
    pub structural_score_distribution: BTreeMap<String, usize>,
    pub motif_complexity_stats: BTreeMap<String, f64>,
    pub rejection_breakdown: RejectionStats,
}

/// Breakdown of candidate rejections by category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionStats {
    pub too_simple: usize,
    pub low_structural_complexity: usize,
    pub high_external_dependency: usize,
    pub excessive_io_operations: usize,
    pub low_motif_diversity: usize,
    pub insufficient_size: usize,
    pub other: usize,
}

impl Default for RejectionStats {
    fn default() -> Self {
        Self {
            too_simple: 0,
            low_structural_complexity: 0,
            high_external_dependency: 0,
            excessive_io_operations: 0,
            low_motif_diversity: 0,
            insufficient_size: 0,
            other: 0,
        }
    }
}

/// IDF (Inverse Document Frequency) statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdfStatistics {
    pub term: String,
    pub document_frequency: usize,
    pub total_documents: usize,
    pub idf_score: f64,
    pub normalized_idf: f64,
    pub term_category: String,
    pub significance_score: f64,
    pub usage_pattern: HashMap<String, f64>,
}

/// Hard filtering floors for quality assurance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardFilteringFloors {
    pub min_saved_tokens: usize,
    pub min_rarity_gain: f64,
    pub min_live_reach_boost: f64,
    pub min_overall_score: f64,
    pub min_confidence: f64,
    pub max_acceptable_noise: f64,
}

impl Default for HardFilteringFloors {
    fn default() -> Self {
        Self {
            min_saved_tokens: 100,
            min_rarity_gain: 1.2,
            min_live_reach_boost: 1.0,
            min_overall_score: 0.6,
            min_confidence: 0.7,
            max_acceptable_noise: 0.2,
        }
    }
}

/// Quality assessment metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub accuracy: f64,
    pub false_positive_rate: f64,
    pub false_negative_rate: f64,
    pub matthews_correlation: f64,
    pub area_under_curve: f64,
    pub confidence_interval_precision: (f64, f64),
    pub confidence_interval_recall: (f64, f64),
    pub sample_size: usize,
    pub validation_method: String,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            precision: 0.0,
            recall: 0.0,
            f1_score: 0.0,
            accuracy: 0.0,
            false_positive_rate: 0.0,
            false_negative_rate: 0.0,
            matthews_correlation: 0.0,
            area_under_curve: 0.0,
            confidence_interval_precision: (0.0, 0.0),
            confidence_interval_recall: (0.0, 0.0),
            sample_size: 0,
            validation_method: "none".to_string(),
        }
    }
}

/// Cached calibration results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCalibration {
    pub threshold: f64,
    pub quality_metrics: QualityMetrics,
    pub noise_metrics: NoiseMetrics,
    pub timestamp: u64,
    pub codebase_signature: String,
    pub calibration_parameters: HashMap<String, f64>,
    pub validation_results: Vec<f64>,
    pub convergence_history: Vec<f64>,
    pub stability_score: f64,
}

/// Type of clone detected
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CloneType {
    Type1, // Exact copies
    Type2, // Syntactically identical with variable/literal differences
    Type3, // Copied code with statements added/removed/modified
    Type4, // Semantic clones with different syntax
}

/// Filtering statistics for transparency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteringStatistics {
    pub filters_applied: Vec<String>,
    pub filter_scores: HashMap<String, f64>,
    pub overall_filter_score: f64,
    pub decision_confidence: f64,
}

impl Default for FilteringStatistics {
    fn default() -> Self {
        Self {
            filters_applied: Vec::new(),
            filter_scores: HashMap::new(),
            overall_filter_score: 0.0,
            decision_confidence: 0.0,
        }
    }
}
