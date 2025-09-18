//! PDG (Program Dependence Graph) Analysis for structural clone detection

// use serde::{Deserialize, Serialize}; // Currently unused
use std::collections::HashMap;
use std::sync::Arc;

use super::types::{BasicBlock, MotifCategory, MotifType, PdgMotif};
use tree_sitter::{Language, Node, Parser, Tree};

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

        let motifs = self.analyze_structure(code);

        // Cache the results
        self.motif_cache
            .insert(entity_id.to_string(), motifs.clone());

        motifs
    }

    fn analyze_structure(&self, code: &str) -> Vec<PdgMotif> {
        if let Some((language, tree, counters)) = parse_with_available_languages(code) {
            let mut counters = counters;
            if counters.score() == 0 {
                collect_motifs(language.as_str(), tree.root_node(), &mut counters);
            }
            counters.into_motifs()
        } else {
            fallback_motifs(code)
        }
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

#[derive(Default)]
struct PdgCounters {
    loops: usize,
    conditionals: usize,
    returns: usize,
    calls: usize,
    matches: usize,
    try_blocks: usize,
}

impl PdgCounters {
    fn score(&self) -> usize {
        self.loops + self.conditionals + self.returns + self.calls + self.matches + self.try_blocks
    }

    fn into_motifs(self) -> Vec<PdgMotif> {
        let mut motifs = Vec::new();

        if self.loops > 0 {
            motifs.push(PdgMotif {
                pattern: format!("LOOP_CONSTRUCT_{}", self.loops.min(5)),
                motif_type: MotifType::Control,
                category: MotifCategory::Loop,
                weight: (self.loops as u32).min(4),
            });
        }

        if self.conditionals > 0 {
            motifs.push(PdgMotif {
                pattern: format!("CONDITIONAL_BRANCHES_{}", self.conditionals.min(5)),
                motif_type: MotifType::Control,
                category: MotifCategory::Conditional,
                weight: (self.conditionals as u32).min(4),
            });
        }

        if self.matches > 0 {
            motifs.push(PdgMotif {
                pattern: "PATTERN_MATCH".to_string(),
                motif_type: MotifType::Control,
                category: MotifCategory::Conditional,
                weight: (self.matches as u32).min(3),
            });
        }

        if self.try_blocks > 0 {
            motifs.push(PdgMotif {
                pattern: "ERROR_HANDLING".to_string(),
                motif_type: MotifType::Control,
                category: MotifCategory::Conditional,
                weight: (self.try_blocks as u32).min(3),
            });
        }

        if self.returns > 0 {
            motifs.push(PdgMotif {
                pattern: "RETURN_FLOW".to_string(),
                motif_type: MotifType::Control,
                category: MotifCategory::Return,
                weight: (self.returns as u32).min(3),
            });
        }

        if self.calls > 0 {
            motifs.push(PdgMotif {
                pattern: format!("CALL_PATTERN_{}", self.calls.min(5)),
                motif_type: MotifType::Control,
                category: MotifCategory::Call,
                weight: (self.calls as u32).min(4),
            });
        }

        motifs
    }
}

fn collect_motifs(language: &str, node: Node, counters: &mut PdgCounters) {
    let kind = node.kind();

    match language {
        "python" => match kind {
            "for_statement" | "while_statement" => counters.loops += 1,
            "if_statement" | "elif_clause" | "conditional_expression" => counters.conditionals += 1,
            "try_statement" => counters.try_blocks += 1,
            "match_statement" => counters.matches += 1,
            "return_statement" => counters.returns += 1,
            "call" => counters.calls += 1,
            _ => {}
        },
        "javascript" | "typescript" => match kind {
            "for_statement" | "while_statement" | "for_in_statement" | "for_of_statement" => {
                counters.loops += 1
            }
            "if_statement" | "switch_statement" | "conditional_expression" => {
                counters.conditionals += 1
            }
            "try_statement" => counters.try_blocks += 1,
            "call_expression" | "new_expression" => counters.calls += 1,
            "return_statement" => counters.returns += 1,
            _ => {}
        },
        "rust" => match kind {
            "for_expression" | "while_expression" | "loop_expression" => counters.loops += 1,
            "if_expression" | "match_expression" => counters.conditionals += 1,
            "try_expression" => counters.try_blocks += 1,
            "call_expression" => counters.calls += 1,
            "return_expression" => counters.returns += 1,
            _ => {}
        },
        "go" => match kind {
            "for_statement" => counters.loops += 1,
            "if_statement" | "switch_statement" => counters.conditionals += 1,
            "type_switch_statement" => counters.matches += 1,
            "call_expression" => counters.calls += 1,
            "return_statement" => counters.returns += 1,
            _ => {}
        },
        _ => {}
    }

    // Some grammars represent calls via identifiers with argument lists.
    if matches!(language, "python" | "javascript" | "typescript") && kind == "identifier" {
        if let Some(next) = node.next_sibling() {
            if next.kind() == "argument_list" {
                counters.calls += 1;
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_motifs(language, child, counters);
    }
}

fn parse_with_available_languages(code: &str) -> Option<(String, Tree, PdgCounters)> {
    let mut best: Option<(String, Tree, PdgCounters)> = None;

    for language in ["python", "javascript", "typescript", "rust", "go"] {
        if let Some(tree) = parse_with_language(language, code) {
            let mut counters = PdgCounters::default();
            collect_motifs(language, tree.root_node(), &mut counters);
            let score = counters.score();

            if score > 0 {
                return Some((language.to_string(), tree, counters));
            }

            if let Some((_, _, best_counters)) = &best {
                if score > best_counters.score() {
                    best = Some((language.to_string(), tree, counters));
                }
            } else {
                best = Some((language.to_string(), tree, counters));
            }
        }
    }

    best
}

fn parse_with_language(language: &str, code: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    let lang = match language {
        "python" => tree_sitter_language("python")?,
        "javascript" => tree_sitter_language("javascript")?,
        "typescript" => tree_sitter_language("typescript")?,
        "rust" => tree_sitter_language("rust")?,
        "go" => tree_sitter_language("go")?,
        _ => return None,
    };
    parser.set_language(&lang).ok()?;
    parser.parse(code, None)
}

fn tree_sitter_language(name: &str) -> Option<Language> {
    unsafe {
        match name {
            "python" => Some(tree_sitter_python::LANGUAGE.into()),
            "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
            "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
            "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
            "go" => Some(tree_sitter_go::LANGUAGE.into()),
            _ => None,
        }
    }
}

fn fallback_motifs(code: &str) -> Vec<PdgMotif> {
    let mut motifs = Vec::new();

    if code.contains("for") || code.contains("while") {
        motifs.push(PdgMotif {
            pattern: "LOOP_CONSTRUCT".to_string(),
            motif_type: MotifType::Control,
            category: MotifCategory::Loop,
            weight: 1,
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
