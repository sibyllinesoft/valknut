//! Clique-style partitioning helpers for similarity pre-filtering.
//!
//! This module builds lightweight lexical graphs over code entities and extracts
//! dense groups that can be fed into expensive similarity detectors (e.g. LSH).
//! The implementation relies on fast token hashing and petgraph traversal, which
//! keeps the preprocessing overhead minimal while dramatically reducing the size
//! of downstream candidate sets.

use std::collections::{HashMap, HashSet, VecDeque};

use tracing::{debug, info};
use xxhash_rust::xxh3::xxh3_64;

use crate::core::featureset::{CodeEntity, EntityId};

/// Mapping from an entity id to the other entity ids that belong to the same
/// candidate clique.
pub type CliquePartitions = HashMap<EntityId, Vec<EntityId>>;

/// Heuristic builder for similarity cliques.
#[derive(Debug, Clone)]
pub struct SimilarityCliquePartitioner {
    min_token_length: usize,
    min_shared_tokens: usize,
    min_jaccard: f64,
    max_token_bucket: usize,
    max_tokens_per_entity: usize,
    max_group_size: usize,
}

/// Default implementation for [`SimilarityCliquePartitioner`].
impl Default for SimilarityCliquePartitioner {
    /// Returns a partitioner with tuned default settings.
    fn default() -> Self {
        Self::new()
    }
}

/// Factory, partitioning, and token extraction methods for [`SimilarityCliquePartitioner`].
impl SimilarityCliquePartitioner {
    /// Common language keywords that are too generic to help with grouping.
    const STOPWORDS: &'static [&'static str] = &[
        "fn",
        "function",
        "def",
        "let",
        "var",
        "const",
        "class",
        "struct",
        "impl",
        "interface",
        "return",
        "true",
        "false",
        "null",
        "none",
        "self",
        "this",
        "int",
        "float",
        "string",
        "bool",
        "public",
        "private",
        "protected",
        "static",
        "async",
        "await",
        "match",
        "loop",
        "while",
        "for",
        "if",
        "else",
        "elif",
        "case",
        "switch",
    ];

    /// Create a new partitioner with tuned defaults.
    pub fn new() -> Self {
        Self {
            min_token_length: 3,
            min_shared_tokens: 2,
            min_jaccard: 0.2,
            max_token_bucket: 256,
            max_tokens_per_entity: 128,
            max_group_size: 48,
        }
    }

    /// Partition the provided entities into candidate cliques.
    pub fn partition(&self, entities: &[CodeEntity]) -> CliquePartitions {
        if entities.len() < 2 {
            return CliquePartitions::new();
        }

        let (token_sets, token_buckets) = self.build_token_sets_and_buckets(entities);
        let (pair_counts, candidate_pairs, skipped_large_buckets) =
            self.count_candidate_pairs(token_buckets, &token_sets);
        let (adjacency, edges_added) =
            self.build_adjacency_graph(pair_counts, &token_sets, entities.len());

        let (partitions, total_group_members, largest_group) =
            self.find_and_register_components(&adjacency, entities);

        let group_count = partitions.len();
        let average_group = if group_count > 0 {
            total_group_members as f64 / group_count as f64
        } else {
            0.0
        };
        info!(
            entities = entities.len(),
            groups = group_count,
            largest_group = largest_group,
            average_group_size = average_group,
            candidate_pairs,
            edges_added,
            skipped_large_buckets,
            "Similarity clique partitioning summary"
        );

        partitions
    }

    /// Build token sets for each entity and inverted index buckets.
    fn build_token_sets_and_buckets(
        &self,
        entities: &[CodeEntity],
    ) -> (Vec<HashSet<u64>>, HashMap<u64, Vec<usize>>) {
        let mut token_sets: Vec<HashSet<u64>> = Vec::with_capacity(entities.len());
        let mut token_buckets: HashMap<u64, Vec<usize>> = HashMap::new();

        for (idx, entity) in entities.iter().enumerate() {
            let tokens = self.extract_tokens(&entity.source_code);
            if tokens.is_empty() {
                token_sets.push(HashSet::new());
                continue;
            }

            for token_hash in &tokens {
                token_buckets.entry(*token_hash).or_default().push(idx);
            }

            token_sets.push(tokens);
        }

        (token_sets, token_buckets)
    }

    /// Count candidate pairs from token buckets.
    /// Returns (pair_counts, candidate_pairs_count, skipped_large_buckets).
    fn count_candidate_pairs(
        &self,
        mut token_buckets: HashMap<u64, Vec<usize>>,
        token_sets: &[HashSet<u64>],
    ) -> (HashMap<(usize, usize), usize>, usize, usize) {
        let mut pair_counts: HashMap<(usize, usize), usize> = HashMap::new();
        let mut skipped_large_buckets = 0usize;
        let mut candidate_pairs = 0usize;

        for indices in token_buckets.values_mut() {
            if indices.len() < 2 {
                continue;
            }
            if indices.len() > self.max_token_bucket {
                skipped_large_buckets += 1;
                continue;
            }
            indices.sort_unstable();
            self.count_pairs_in_bucket(indices, token_sets, &mut pair_counts, &mut candidate_pairs);
        }

        (pair_counts, candidate_pairs, skipped_large_buckets)
    }

    /// Count pairs within a single bucket (extracted to reduce nesting).
    fn count_pairs_in_bucket(
        &self,
        indices: &[usize],
        token_sets: &[HashSet<u64>],
        pair_counts: &mut HashMap<(usize, usize), usize>,
        candidate_pairs: &mut usize,
    ) {
        for i in 0..indices.len() {
            for j in (i + 1)..indices.len() {
                let a = indices[i];
                let b = indices[j];
                if token_sets[a].is_empty() || token_sets[b].is_empty() {
                    continue;
                }
                *candidate_pairs += 1;
                *pair_counts.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
    }

    /// Build adjacency graph from pair counts using jaccard similarity filtering.
    fn build_adjacency_graph(
        &self,
        pair_counts: HashMap<(usize, usize), usize>,
        token_sets: &[HashSet<u64>],
        entity_count: usize,
    ) -> (Vec<Vec<usize>>, usize) {
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); entity_count];
        let mut edges_added = 0usize;

        for ((i, j), shared) in pair_counts {
            let set_i_len = token_sets[i].len();
            let set_j_len = token_sets[j].len();
            if shared < self.min_shared_tokens || set_i_len == 0 || set_j_len == 0 {
                continue;
            }

            let union = set_i_len + set_j_len - shared;
            if union == 0 {
                continue;
            }

            let jaccard = shared as f64 / union as f64;
            if jaccard >= self.min_jaccard {
                adjacency[i].push(j);
                adjacency[j].push(i);
                edges_added += 1;
            }
        }

        (adjacency, edges_added)
    }

    /// Find connected components via BFS and register them as partitions.
    fn find_and_register_components(
        &self,
        adjacency: &[Vec<usize>],
        entities: &[CodeEntity],
    ) -> (CliquePartitions, usize, usize) {
        let mut visited = vec![false; entities.len()];
        let mut partitions = CliquePartitions::new();
        let mut total_group_members = 0usize;
        let mut largest_group = 0usize;

        for start in 0..entities.len() {
            if visited[start] {
                continue;
            }

            let component = self.bfs_component(start, adjacency, &mut visited);
            if component.len() <= 1 {
                continue;
            }

            largest_group = largest_group.max(component.len());
            total_group_members += component.len();

            self.register_component(&component, entities, &mut partitions);
        }

        (partitions, total_group_members, largest_group)
    }

    /// Perform BFS to find a connected component starting from `start`.
    fn bfs_component(
        &self,
        start: usize,
        adjacency: &[Vec<usize>],
        visited: &mut [bool],
    ) -> Vec<usize> {
        visited[start] = true;
        let mut queue = VecDeque::new();
        queue.push_back(start);
        let mut component = Vec::new();

        while let Some(current) = queue.pop_front() {
            component.push(current);
            for &neigh in &adjacency[current] {
                if !visited[neigh] {
                    visited[neigh] = true;
                    queue.push_back(neigh);
                }
            }
        }

        component.sort_unstable();
        component
    }

    /// Register a component as partition(s), splitting large ones into chunks.
    fn register_component(
        &self,
        component: &[usize],
        entities: &[CodeEntity],
        partitions: &mut CliquePartitions,
    ) {
        let ids: Vec<String> = component.iter().map(|&idx| entities[idx].id.clone()).collect();

        if component.len() > self.max_group_size {
            // Break large components into deterministic chunks so that the
            // downstream stages never explode in complexity.
            let mut sorted_ids = ids;
            sorted_ids.sort();
            for chunk in sorted_ids.chunks(self.max_group_size) {
                if chunk.len() > 1 {
                    self.register_group(chunk, partitions);
                }
            }
        } else {
            self.register_group(&ids, partitions);
        }
    }

    /// Extracts identifier tokens from source code as hashed values.
    fn extract_tokens(&self, source: &str) -> HashSet<u64> {
        if source.trim().is_empty() {
            return HashSet::new();
        }

        let mut tokens = HashSet::new();
        let mut current = String::new();

        for ch in source.chars() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                current.push(ch.to_ascii_lowercase());
                if current.len() >= 64 {
                    self.try_store_token(&mut tokens, &current);
                    current.clear();
                }
            } else if !current.is_empty() {
                self.try_store_token(&mut tokens, &current);
                current.clear();
                if tokens.len() >= self.max_tokens_per_entity {
                    break;
                }
            }
        }

        if !current.is_empty() && tokens.len() < self.max_tokens_per_entity {
            self.try_store_token(&mut tokens, &current);
        }

        tokens
    }

    /// Attempts to store a token hash if it meets length and stopword criteria.
    fn try_store_token(&self, tokens: &mut HashSet<u64>, token: &str) {
        if token.len() < self.min_token_length {
            return;
        }

        let normalized = token.trim_matches('_');
        if normalized.len() < self.min_token_length {
            return;
        }

        if Self::STOPWORDS.iter().any(|&stop| stop == normalized) {
            return;
        }

        if !normalized
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return;
        }

        if tokens.len() >= self.max_tokens_per_entity {
            return;
        }

        let hash = xxh3_64(normalized.as_bytes());
        tokens.insert(hash);
    }

    /// Registers a group of entity IDs as belonging to the same clique.
    fn register_group(&self, group: &[String], partitions: &mut CliquePartitions) {
        for (idx, entity_id) in group.iter().enumerate() {
            let mut others = Vec::with_capacity(group.len().saturating_sub(1));
            for (other_idx, other_id) in group.iter().enumerate() {
                if idx == other_idx {
                    continue;
                }
                others.push(other_id.clone());
            }
            partitions.insert(entity_id.clone(), others);
        }
    }

    /// Expose the configured maximum group size (useful for tests and tuning).
    pub fn max_group_size(&self) -> usize {
        self.max_group_size
    }

    /// Override the maximum group size (primarily for testing scenarios).
    #[cfg(test)]
    pub fn with_max_group_size(mut self, size: usize) -> Self {
        self.max_group_size = size.max(2);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::SimilarityCliquePartitioner;
    use crate::core::featureset::CodeEntity;

    fn make_entity(id: &str, body: &str) -> CodeEntity {
        CodeEntity::new(id, "function", id, "test.rs").with_source_code(body)
    }

    #[test]
    fn partitions_similar_entities() {
        let entities = vec![
            make_entity(
                "a",
                "fn process_order(order: Order) { validate_order(&order); calculate_total(&order); finalize(order) }",
            ),
            make_entity(
                "b",
                "fn handle_order(order: Order) { validate_order(&order); calculate_total(&order); finalize(order) }",
            ),
            make_entity("c", "fn greet() { println!(\"hi\"); }"),
        ];

        let partitioner = SimilarityCliquePartitioner::new();
        let partitions = partitioner.partition(&entities);

        assert!(partitions
            .get("a")
            .map_or(false, |group| group.iter().any(|id| id == "b")));
        assert!(partitions
            .get("b")
            .map_or(false, |group| group.iter().any(|id| id == "a")));
        assert!(partitions.get("c").is_none());
    }

    #[test]
    fn splits_large_components() {
        let mut entities = Vec::new();
        for idx in 0..12 {
            let code = format!(
                "fn handler_{}(order: &Order) {{ validate_order(order); normalize_order(order); persist_order(order); finalize_order(order); }}",
                idx
            );
            entities.push(make_entity(&format!("h{}", idx), &code));
        }

        let partitioner = SimilarityCliquePartitioner::new().with_max_group_size(5);
        let partitions = partitioner.partition(&entities);

        assert!(!partitions.is_empty());
        let max_group = partitions
            .values()
            .map(|group| group.len())
            .max()
            .unwrap_or(0);
        assert!(max_group <= partitioner.max_group_size());
    }
}
