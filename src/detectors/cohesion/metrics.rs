//! Cohesion metrics calculation including robust centroid and outlier detection.

use super::config::{CohesionConfig, WeightFunction};

/// Calculator for cohesion metrics.
pub struct CohesionCalculator {
    /// Minimum entities for file cohesion
    min_file_entities: usize,
    /// Minimum files for folder cohesion
    min_folder_files: usize,
    /// Centroid trim percentage
    trim_percent: f64,
    /// Whether to use MAD-based trimming
    use_mad: bool,
    /// MAD multiplier
    mad_multiplier: f64,
    /// File weight function
    file_weight_fn: WeightFunction,
}

/// Factory and cohesion calculation methods for [`CohesionCalculator`].
impl CohesionCalculator {
    /// Create a new calculator from configuration.
    pub fn new(config: &CohesionConfig) -> Self {
        Self {
            min_file_entities: config.rollup.min_file_entities,
            min_folder_files: config.rollup.min_folder_files,
            trim_percent: config.rollup.centroid_trim_percent,
            use_mad: config.rollup.use_mad_trimming,
            mad_multiplier: config.rollup.mad_multiplier,
            file_weight_fn: config.rollup.file_weight_function,
        }
    }

    /// Calculate file weight for folder rollup.
    pub fn file_weight(&self, entity_count: usize) -> f64 {
        self.file_weight_fn.weight(entity_count)
    }

    /// Check if file has enough entities for cohesion analysis.
    pub fn file_eligible(&self, entity_count: usize) -> bool {
        entity_count >= self.min_file_entities
    }

    /// Check if folder has enough files for cohesion analysis.
    pub fn folder_eligible(&self, file_count: usize) -> bool {
        file_count >= self.min_folder_files
    }

    /// Calculate robust centroid from embeddings.
    ///
    /// Uses 2-pass algorithm:
    /// 1. Initial centroid from all embeddings
    /// 2. Trim bottom X% by similarity or use MAD-based trimming
    /// 3. Recalculate centroid from remaining embeddings
    pub fn robust_centroid(&self, embeddings: &[Vec<f32>]) -> Option<Vec<f32>> {
        if embeddings.is_empty() {
            return None;
        }

        if embeddings.len() == 1 {
            return Some(normalize(&embeddings[0]));
        }

        // Pass 1: Initial centroid
        let initial = self.simple_centroid(embeddings)?;

        // Calculate similarities to initial centroid
        let mut similarities: Vec<(usize, f64)> = embeddings
            .iter()
            .enumerate()
            .map(|(i, e)| (i, cosine_similarity(e, &initial)))
            .collect();

        // Sort by similarity ascending (lowest first)
        similarities.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Determine which to keep
        let indices_to_keep: Vec<usize> = if self.use_mad {
            // MAD-based trimming
            let sims: Vec<f64> = similarities.iter().map(|(_, s)| *s).collect();
            let median = median_of(&sims);
            let mad = median_absolute_deviation(&sims, median);
            let threshold = median - self.mad_multiplier * mad;

            similarities
                .iter()
                .filter(|(_, s)| *s >= threshold)
                .map(|(i, _)| *i)
                .collect()
        } else {
            // Percentile-based trimming
            let trim_count = ((embeddings.len() as f64) * self.trim_percent).ceil() as usize;
            let keep_count = embeddings.len().saturating_sub(trim_count).max(1);

            similarities
                .iter()
                .skip(embeddings.len() - keep_count) // Skip the lowest similarity ones
                .map(|(i, _)| *i)
                .collect()
        };

        if indices_to_keep.is_empty() {
            return Some(initial);
        }

        // Pass 2: Centroid from kept embeddings
        let kept_embeddings: Vec<&Vec<f32>> = indices_to_keep
            .iter()
            .map(|&i| &embeddings[i])
            .collect();

        self.simple_centroid_refs(&kept_embeddings)
    }

    /// Calculate simple (non-robust) centroid.
    fn simple_centroid(&self, embeddings: &[Vec<f32>]) -> Option<Vec<f32>> {
        if embeddings.is_empty() {
            return None;
        }

        let dim = embeddings[0].len();
        let mut sum = vec![0.0f32; dim];

        for emb in embeddings {
            for (i, &val) in emb.iter().enumerate() {
                sum[i] += val;
            }
        }

        Some(normalize(&sum))
    }

    /// Calculate simple centroid from references.
    fn simple_centroid_refs(&self, embeddings: &[&Vec<f32>]) -> Option<Vec<f32>> {
        if embeddings.is_empty() {
            return None;
        }

        let dim = embeddings[0].len();
        let mut sum = vec![0.0f32; dim];

        for emb in embeddings {
            for (i, &val) in emb.iter().enumerate() {
                sum[i] += val;
            }
        }

        Some(normalize(&sum))
    }

    /// Calculate cohesion score from embeddings.
    ///
    /// Cohesion = ||S|| / n where S = Σ e_i (sum of normalized embeddings)
    /// Equivalently, this is the mean cosine similarity to centroid.
    pub fn cohesion_score(&self, embeddings: &[Vec<f32>]) -> f64 {
        if embeddings.len() < 2 {
            return 1.0; // Single entity is perfectly cohesive with itself
        }

        let n = embeddings.len() as f64;

        // Sum all normalized embeddings
        let dim = embeddings[0].len();
        let mut sum = vec![0.0f64; dim];

        for emb in embeddings {
            let normed = normalize(emb);
            for (i, &val) in normed.iter().enumerate() {
                sum[i] += val as f64;
            }
        }

        // ||S|| / n
        let norm: f64 = sum.iter().map(|x| x * x).sum::<f64>().sqrt();
        (norm / n).clamp(0.0, 1.0)
    }

    /// Calculate doc-code alignment score.
    pub fn doc_alignment(&self, doc_embedding: &[f32], code_centroid: &[f32]) -> f64 {
        cosine_similarity(doc_embedding, code_centroid).clamp(0.0, 1.0)
    }

    /// Find outliers among embeddings relative to a centroid.
    ///
    /// Returns indices of embeddings that are outliers (low similarity to centroid).
    pub fn find_outliers(
        &self,
        embeddings: &[Vec<f32>],
        centroid: &[f32],
        percentile_threshold: f64,
        min_similarity: f64,
    ) -> Vec<(usize, f64)> {
        if embeddings.len() < 3 {
            return Vec::new(); // Too few to have meaningful outliers
        }

        let mut similarities: Vec<(usize, f64)> = embeddings
            .iter()
            .enumerate()
            .map(|(i, e)| (i, cosine_similarity(e, centroid)))
            .collect();

        // Sort by similarity ascending
        similarities.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Find threshold at given percentile
        let percentile_idx =
            ((embeddings.len() as f64) * percentile_threshold).floor() as usize;
        let percentile_sim = similarities
            .get(percentile_idx)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);

        // Use the more restrictive of percentile and absolute threshold
        let threshold = percentile_sim.max(min_similarity);

        // Return all below threshold
        similarities
            .into_iter()
            .filter(|(_, sim)| *sim < threshold)
            .collect()
    }

    /// Calculate weighted rollup sum for folder aggregation.
    ///
    /// Each file contributes: w_file * S_file where w = weight_fn(entity_count)
    pub fn weighted_rollup(
        &self,
        file_sums: &[(usize, Vec<f32>)], // (entity_count, sum vector)
    ) -> Option<(usize, Vec<f32>)> {
        if file_sums.is_empty() {
            return None;
        }

        let dim = file_sums[0].1.len();
        let mut total_sum = vec![0.0f32; dim];
        let mut total_n = 0usize;

        for (n, sum) in file_sums {
            let weight = self.file_weight(*n) as f32;
            total_n += n;

            for (i, &val) in sum.iter().enumerate() {
                total_sum[i] += weight * val;
            }
        }

        Some((total_n, total_sum))
    }
}

/// Normalize a vector to unit length.
pub fn normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-10 {
        v.to_vec()
    } else {
        v.iter().map(|x| x / norm).collect()
    }
}

/// Calculate cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f64 = a.iter().zip(b.iter()).map(|(&x, &y)| x as f64 * y as f64).sum();
    let norm_a: f64 = a.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>().sqrt();
    let norm_b: f64 = b.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>().sqrt();

    if norm_a < 1e-10 || norm_b < 1e-10 {
        0.0
    } else {
        (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
    }
}

/// Calculate median of a slice.
fn median_of(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Calculate median absolute deviation.
fn median_absolute_deviation(values: &[f64], median: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let deviations: Vec<f64> = values.iter().map(|&v| (v - median).abs()).collect();
    median_of(&deviations)
}

/// Rollup state for hierarchical aggregation.
#[derive(Debug, Clone)]
pub struct RollupState {
    /// Number of child embeddings (for entity-level)
    pub n: usize,
    /// Sum of weights (for folder-level weighted aggregation)
    pub weight_sum: f64,
    /// Sum of normalized embeddings
    pub sum: Vec<f32>,
}

/// Factory and aggregation methods for [`RollupState`].
impl RollupState {
    /// Create empty rollup state.
    pub fn new(dimension: usize) -> Self {
        Self {
            n: 0,
            weight_sum: 0.0,
            sum: vec![0.0; dimension],
        }
    }

    /// Add an embedding to the rollup.
    pub fn add(&mut self, embedding: &[f32]) {
        let normed = normalize(embedding);
        self.n += 1;
        self.weight_sum += 1.0; // Each entity has weight 1
        for (i, &val) in normed.iter().enumerate() {
            if i < self.sum.len() {
                self.sum[i] += val;
            }
        }
    }

    /// Add another rollup state (for folder aggregation).
    ///
    /// Uses the normalized centroid of the other state, weighted by the given weight.
    /// This ensures folder cohesion properly measures how similar file centroids are.
    pub fn add_rollup(&mut self, other: &RollupState, weight: f32) {
        self.n += other.n;
        self.weight_sum += weight as f64; // Track actual weight used

        // Use normalized centroid, not raw sum, for proper folder cohesion calculation
        let centroid = other.centroid();
        for (i, &val) in centroid.iter().enumerate() {
            if i < self.sum.len() {
                self.sum[i] += weight * val;
            }
        }
    }

    /// Get the centroid (normalized sum).
    pub fn centroid(&self) -> Vec<f32> {
        normalize(&self.sum)
    }

    /// Get cohesion score.
    ///
    /// For entity-level rollup: ||S|| / n where n = entity count
    /// For folder-level rollup: ||S|| / weight_sum to account for weighted sums
    pub fn cohesion(&self) -> f64 {
        // Use weight_sum for proper scaling (handles both entity-level and folder-level)
        if self.weight_sum < 2.0 {
            return 1.0;
        }

        let norm: f64 = self.sum.iter().map(|&x| (x as f64) * (x as f64)).sum::<f64>().sqrt();
        let cohesion = (norm / self.weight_sum).clamp(0.0, 1.0);

        cohesion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> CohesionConfig {
        CohesionConfig::default()
    }

    #[test]
    fn normalize_unit_vector() {
        let v = vec![1.0, 0.0, 0.0];
        let normed = normalize(&v);
        assert!((normed[0] - 1.0).abs() < 1e-6);
        assert!(normed[1].abs() < 1e-6);
    }

    #[test]
    fn normalize_non_unit_vector() {
        let v = vec![3.0, 4.0];
        let normed = normalize(&v);
        let expected_norm: f32 = normed.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((expected_norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn cohesion_score_identical_vectors() {
        let calc = CohesionCalculator::new(&make_config());
        let embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
            vec![1.0, 0.0, 0.0],
        ];
        let score = calc.cohesion_score(&embeddings);
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cohesion_score_orthogonal_vectors() {
        let calc = CohesionCalculator::new(&make_config());
        let embeddings = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let score = calc.cohesion_score(&embeddings);
        // Sum = [1,1,1], ||sum|| = sqrt(3), score = sqrt(3)/3 ≈ 0.577
        assert!(score > 0.5 && score < 0.6);
    }

    #[test]
    fn robust_centroid_handles_outlier() {
        let mut config = make_config();
        config.rollup.centroid_trim_percent = 0.25; // Trim 25%
        let calc = CohesionCalculator::new(&config);

        let embeddings = vec![
            vec![1.0, 0.0],
            vec![0.9, 0.1],
            vec![0.95, 0.05],
            vec![-1.0, 0.0], // Outlier
        ];

        let centroid = calc.robust_centroid(&embeddings).unwrap();
        // Centroid should be closer to the cluster of similar vectors
        assert!(centroid[0] > 0.9);
    }

    #[test]
    fn find_outliers_returns_low_similarity() {
        let calc = CohesionCalculator::new(&make_config());

        let embeddings = vec![
            vec![1.0, 0.0],
            vec![0.99, 0.1],
            vec![0.98, 0.2],
            vec![-0.5, 0.5], // Low similarity to centroid
        ];

        let centroid = vec![1.0, 0.0];
        let outliers = calc.find_outliers(&embeddings, &centroid, 0.25, 0.3);

        assert_eq!(outliers.len(), 1);
        assert_eq!(outliers[0].0, 3); // Index 3 is the outlier
    }

    #[test]
    fn rollup_state_accumulates() {
        let mut state = RollupState::new(3);

        state.add(&[1.0, 0.0, 0.0]);
        state.add(&[0.0, 1.0, 0.0]);

        assert_eq!(state.n, 2);
        // Sum should be approximately [1, 1, 0] (unnormalized)
        let centroid = state.centroid();
        assert!(centroid[0] > 0.0);
        assert!(centroid[1] > 0.0);
        assert!(centroid[2].abs() < 0.01);
    }

    #[test]
    fn rollup_state_cohesion() {
        let mut state = RollupState::new(2);

        // Add same vector twice - perfect cohesion
        state.add(&[1.0, 0.0]);
        state.add(&[1.0, 0.0]);

        assert!((state.cohesion() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn median_calculation() {
        assert!((median_of(&[1.0, 2.0, 3.0]) - 2.0).abs() < 1e-6);
        assert!((median_of(&[1.0, 2.0, 3.0, 4.0]) - 2.5).abs() < 1e-6);
    }
}
