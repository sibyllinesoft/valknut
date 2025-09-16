//! PDG (Program Dependence Graph) Analysis for structural clone detection

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::types::{BasicBlock, MotifCategory, MotifType, PdgMotif, StructuralMatchInfo};

/// PDG (Program Dependence Graph) Motif Analyzer
#[derive(Debug)]
pub struct PdgMotifAnalyzer {
    /// Cache of analyzed motifs
    motif_cache: HashMap<String, Vec<PdgMotif>>,

    /// Stop-motifs cache for filtering
    stop_motif_cache: Option<Arc<crate::io::cache::StopMotifCache>>,
}

impl PdgMotifAnalyzer {
    /// Create a new PDG motif analyzer
    pub fn new() -> Self {
        Self {
            motif_cache: HashMap::new(),
            stop_motif_cache: None,
        }
    }

    /// Set the stop-motifs cache
    pub fn with_cache(mut self, cache: Arc<crate::io::cache::StopMotifCache>) -> Self {
        self.stop_motif_cache = Some(cache);
        self
    }

    /// Extract PDG motifs from code
    pub fn extract_motifs(&mut self, code: &str, entity_id: &str) -> Vec<PdgMotif> {
        // Check cache first
        if let Some(cached) = self.motif_cache.get(entity_id) {
            return cached.clone();
        }

        // Simplified motif extraction
        let motifs = self.analyze_structure(code);

        // Cache the results
        self.motif_cache
            .insert(entity_id.to_string(), motifs.clone());

        motifs
    }

    /// Analyze structural patterns in code
    fn analyze_structure(&self, code: &str) -> Vec<PdgMotif> {
        let mut motifs = Vec::new();

        // Simple pattern detection (this would be much more sophisticated in practice)
        if code.contains("for") || code.contains("while") {
            motifs.push(PdgMotif {
                pattern: "LOOP_CONSTRUCT".to_string(),
                motif_type: MotifType::Control,
                category: MotifCategory::Loop,
                weight: 2,
            });
        }

        if code.contains("if") {
            motifs.push(PdgMotif {
                pattern: "CONDITIONAL".to_string(),
                motif_type: MotifType::Control,
                category: MotifCategory::Conditional,
                weight: 1,
            });
        }

        if code.contains("return") {
            motifs.push(PdgMotif {
                pattern: "RETURN_STMT".to_string(),
                motif_type: MotifType::Control,
                category: MotifCategory::Return,
                weight: 1,
            });
        }

        // Count function calls
        let call_count = code.matches('(').count();
        if call_count > 0 {
            motifs.push(PdgMotif {
                pattern: format!("CALL_PATTERN_{}", call_count.min(5)),
                motif_type: MotifType::Control,
                category: MotifCategory::Call,
                weight: (call_count as u32).min(3),
            });
        }

        motifs
    }
}

/// Basic block analyzer for structural validation
#[derive(Debug)]
pub struct BasicBlockAnalyzer {
    /// Configuration for analysis
    config: BasicBlockConfig,
}

impl BasicBlockAnalyzer {
    /// Create a new basic block analyzer
    pub fn new() -> Self {
        Self {
            config: BasicBlockConfig::default(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: BasicBlockConfig) -> Self {
        Self { config }
    }

    /// Analyze basic blocks in code
    pub fn analyze_basic_blocks(&self, code: &str) -> Vec<BasicBlock> {
        // Simplified basic block analysis
        let lines: Vec<&str> = code.lines().collect();
        let mut blocks = Vec::new();

        let mut current_block = Vec::new();
        let mut block_id = 0;

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            current_block.push(trimmed.to_string());

            // End block on control flow statements
            if self.is_block_terminator(trimmed) {
                if !current_block.is_empty() {
                    blocks.push(self.create_basic_block(block_id, current_block.clone(), line_num));
                    current_block.clear();
                    block_id += 1;
                }
            }
        }

        // Add final block if not empty
        if !current_block.is_empty() {
            blocks.push(self.create_basic_block(block_id, current_block, lines.len()));
        }

        blocks
    }

    /// Check if a line terminates a basic block
    fn is_block_terminator(&self, line: &str) -> bool {
        line.contains("return")
            || line.contains("break")
            || line.contains("continue")
            || line.contains("goto")
            || line.ends_with('}')
            || line.ends_with(';')
                && (line.contains("if") || line.contains("for") || line.contains("while"))
    }

    /// Create a basic block from statements
    fn create_basic_block(
        &self,
        id: usize,
        statements: Vec<String>,
        end_line: usize,
    ) -> BasicBlock {
        let contains_call = statements.iter().any(|s| s.contains('('));
        let contains_return = statements.iter().any(|s| s.contains("return"));
        let is_loop = statements
            .iter()
            .any(|s| s.contains("for") || s.contains("while"));

        BasicBlock {
            id: format!("bb_{}", id),
            statements,
            successors: Vec::new(), // Would be computed in full analysis
            predecessors: Vec::new(),
            dominance_level: 0,
            loop_depth: if is_loop { 1 } else { 0 },
            control_dependencies: Vec::new(),
            data_dependencies: Vec::new(),
            region_id: None,
            is_loop_header: is_loop,
            is_loop_exit: false,
            contains_call,
            contains_return,
            estimated_execution_frequency: 1.0,
        }
    }
}

/// Configuration for basic block analysis
#[derive(Debug, Clone)]
pub struct BasicBlockConfig {
    pub include_empty_blocks: bool,
    pub merge_sequential_blocks: bool,
    pub compute_dominance: bool,
    pub analyze_dependencies: bool,
}

impl Default for BasicBlockConfig {
    fn default() -> Self {
        Self {
            include_empty_blocks: false,
            merge_sequential_blocks: true,
            compute_dominance: false,
            analyze_dependencies: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdg_motif_analyzer() {
        let mut analyzer = PdgMotifAnalyzer::new();
        let code = "if (x > 0) { for (int i = 0; i < 10; i++) { return i; } }";
        let motifs = analyzer.extract_motifs(code, "test_entity");

        assert!(!motifs.is_empty());
        assert!(motifs
            .iter()
            .any(|m| m.category == MotifCategory::Conditional));
        assert!(motifs.iter().any(|m| m.category == MotifCategory::Loop));
        assert!(motifs.iter().any(|m| m.category == MotifCategory::Return));
    }

    #[test]
    fn test_basic_block_analyzer() {
        let analyzer = BasicBlockAnalyzer::new();
        let code = "x = 1;\ny = 2;\nif (x > y) {\n    return x;\n} else {\n    return y;\n}";
        let blocks = analyzer.analyze_basic_blocks(code);

        assert!(!blocks.is_empty());
        assert!(blocks.iter().any(|b| b.contains_return));
    }

    #[test]
    fn test_motif_caching() {
        let mut analyzer = PdgMotifAnalyzer::new();
        let code = "for (int i = 0; i < 10; i++) { }";

        let motifs1 = analyzer.extract_motifs(code, "test");
        let motifs2 = analyzer.extract_motifs(code, "test");

        // Should return cached results
        assert_eq!(motifs1.len(), motifs2.len());
    }
}
