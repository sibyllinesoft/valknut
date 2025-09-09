//! File analysis, entity extraction, and file splitting logic

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use petgraph::graph::NodeIndex;

use crate::core::errors::Result;
use crate::core::file_utils::FileReader;

use super::config::{
    StructureConfig, FileSplitPack, SuggestedSplit, SplitValue, SplitEffort,
    CohesionGraph, EntityNode, CohesionEdge, ImportStatement
};

pub struct FileAnalyzer {
    config: StructureConfig,
}

impl FileAnalyzer {
    pub fn new(config: StructureConfig) -> Self {
        Self { config }
    }

    /// Check if file extension indicates a code file
    pub fn is_code_file(&self, extension: &str) -> bool {
        matches!(extension, "py" | "js" | "ts" | "jsx" | "tsx" | "rs" | "go" | "java" | "cpp" | "c" | "h" | "hpp")
    }

    /// Count lines of code in a file
    pub fn count_lines_of_code(&self, file_path: &Path) -> Result<usize> {
        FileReader::count_lines_of_code(file_path)
    }

    /// Analyze file for split potential
    pub fn analyze_file_for_split(&self, file_path: &Path) -> Result<Option<FileSplitPack>> {
        let metadata = std::fs::metadata(file_path)?;
        let size_bytes = metadata.len() as usize;
        let loc = self.count_lines_of_code(file_path)?;
        
        // Check if file meets "huge" criteria
        let is_huge = loc >= self.config.fsfile.huge_loc || size_bytes >= self.config.fsfile.huge_bytes;
        
        if !is_huge {
            return Ok(None);
        }

        let mut reasons = Vec::new();
        
        if loc >= self.config.fsfile.huge_loc {
            reasons.push(format!("loc {} > {}", loc, self.config.fsfile.huge_loc));
        }
        
        if size_bytes >= self.config.fsfile.huge_bytes {
            reasons.push(format!("size {} bytes > {} bytes", size_bytes, self.config.fsfile.huge_bytes));
        }

        // Build entity cohesion graph
        let cohesion_graph = self.build_entity_cohesion_graph(file_path)?;
        let communities = self.find_cohesion_communities(&cohesion_graph)?;
        
        if communities.len() >= self.config.partitioning.min_clusters {
            reasons.push(format!("{} cohesion communities", communities.len()));
        } else {
            return Ok(None); // Not worth splitting
        }

        // Generate split suggestions
        let suggested_splits = self.generate_split_suggestions(file_path, &communities)?;
        
        // Calculate value and effort
        let value = self.calculate_split_value(loc, file_path)?;
        let effort = self.calculate_split_effort(file_path)?;

        let pack = FileSplitPack {
            kind: "file_split".to_string(),
            file: file_path.to_path_buf(),
            reasons,
            suggested_splits,
            value,
            effort,
        };

        Ok(Some(pack))
    }

    /// Build entity cohesion graph for file
    pub fn build_entity_cohesion_graph(&self, file_path: &Path) -> Result<CohesionGraph> {
        let mut graph = petgraph::Graph::new_undirected();
        let content = FileReader::read_to_string(file_path)?;
        
        // Extract entities based on file type
        let entities = if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            match ext {
                "py" => self.extract_python_entities(&content)?,
                "js" | "ts" | "jsx" | "tsx" => self.extract_javascript_entities(&content)?,
                "rs" => self.extract_rust_entities(&content)?,
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        };
        
        if entities.len() < 2 {
            return Ok(graph); // Need at least 2 entities for cohesion analysis
        }
        
        // Add entity nodes to graph
        let mut entity_nodes = Vec::new();
        for entity in entities {
            let node_idx = graph.add_node(entity);
            entity_nodes.push(node_idx);
        }
        
        // Calculate cohesion between all pairs of entities
        for i in 0..entity_nodes.len() {
            for j in i + 1..entity_nodes.len() {
                let entity_a = &graph[entity_nodes[i]];
                let entity_b = &graph[entity_nodes[j]];
                
                let jaccard_similarity = self.calculate_jaccard_similarity(&entity_a.symbols, &entity_b.symbols);
                
                // Only add edges for significant cohesion
                if jaccard_similarity > 0.1 {
                    let shared_symbols = entity_a.symbols.intersection(&entity_b.symbols).count();
                    let edge = CohesionEdge {
                        similarity: jaccard_similarity,
                        shared_symbols,
                    };
                    
                    graph.add_edge(entity_nodes[i], entity_nodes[j], edge);
                }
            }
        }
        
        Ok(graph)
    }

    /// Find cohesion communities in entity graph
    pub fn find_cohesion_communities(&self, graph: &CohesionGraph) -> Result<Vec<Vec<NodeIndex>>> {
        let node_indices: Vec<_> = graph.node_indices().collect();
        
        if node_indices.len() < 2 {
            return Ok(vec![node_indices]);
        }
        
        // Use a simple but effective community detection based on edge weights
        let mut communities: Vec<Vec<NodeIndex>> = Vec::new();
        let mut assigned_nodes = HashSet::new();
        
        // Start with the highest cohesion edges and build communities
        let mut edges: Vec<_> = graph.edge_indices()
            .map(|edge_idx| {
                let (source, target) = graph.edge_endpoints(edge_idx).unwrap();
                let weight = graph.edge_weight(edge_idx).unwrap();
                (edge_idx, source, target, weight.similarity)
            })
            .collect();
        
        // Sort by cohesion strength (descending)
        edges.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        
        // Build communities greedily
        for (_, source, target, similarity) in edges {
            if similarity < 0.2 {
                break; // Stop at low similarity threshold
            }
            
            // Find existing communities for these nodes
            let mut source_comm_idx = None;
            let mut target_comm_idx = None;
            
            for (idx, comm) in communities.iter().enumerate() {
                if comm.contains(&source) {
                    source_comm_idx = Some(idx);
                }
                if comm.contains(&target) {
                    target_comm_idx = Some(idx);
                }
            }
            
            match (source_comm_idx, target_comm_idx) {
                (Some(comm_idx), None) => {
                    if !assigned_nodes.contains(&target) {
                        communities[comm_idx].push(target);
                        assigned_nodes.insert(target);
                    }
                },
                (None, Some(comm_idx)) => {
                    if !assigned_nodes.contains(&source) {
                        communities[comm_idx].push(source);
                        assigned_nodes.insert(source);
                    }
                },
                (None, None) => {
                    // Create new community
                    let mut new_community = Vec::new();
                    if !assigned_nodes.contains(&source) {
                        new_community.push(source);
                        assigned_nodes.insert(source);
                    }
                    if !assigned_nodes.contains(&target) {
                        new_community.push(target);
                        assigned_nodes.insert(target);
                    }
                    if !new_community.is_empty() {
                        communities.push(new_community);
                    }
                },
                (Some(_), Some(_)) => {
                    // Both nodes already in communities - could merge but skip for simplicity
                }
            }
        }
        
        // Add any remaining nodes as singleton communities
        for node in node_indices {
            if !assigned_nodes.contains(&node) {
                communities.push(vec![node]);
            }
        }
        
        // Filter out communities that are too small to be meaningful
        communities.retain(|comm| comm.len() >= self.config.fsfile.min_entities_per_split);
        
        // Limit to reasonable number of communities (2-3 for splitting)
        communities.truncate(3);
        
        Ok(communities)
    }

    /// Generate split file suggestions
    pub fn generate_split_suggestions(
        &self,
        file_path: &Path,
        communities: &[Vec<NodeIndex>]
    ) -> Result<Vec<SuggestedSplit>> {
        let cohesion_graph = self.build_entity_cohesion_graph(file_path)?;
        
        let base_name = file_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
            
        let suffixes = ["_core", "_io", "_api"];
        let mut splits = Vec::new();
        
        for (community_idx, community) in communities.iter().enumerate().take(3) {
            let suffix = suffixes.get(community_idx).unwrap_or(&"_part");
            
            let mut entities = Vec::new();
            let mut total_loc = 0;
            
            // Extract entity information from the community
            for &node_idx in community {
                if let Some(entity) = cohesion_graph.node_weight(node_idx) {
                    entities.push(entity.name.clone());
                    total_loc += entity.loc;
                }
            }
            
            // Generate meaningful name based on entity analysis
            let split_name = self.generate_split_name(base_name, suffix, &entities, file_path);
            
            splits.push(SuggestedSplit {
                name: split_name,
                entities,
                loc: total_loc,
            });
        }
        
        // If no communities found, create default splits
        if splits.is_empty() {
            for (i, suffix) in suffixes.iter().enumerate().take(2) {
                splits.push(SuggestedSplit {
                    name: format!("{}{}.{}", base_name, suffix, 
                        file_path.extension().and_then(|e| e.to_str()).unwrap_or("py")),
                    entities: vec![format!("Entity{}", i + 1)],
                    loc: 400, // Rough estimate
                });
            }
        }
        
        Ok(splits)
    }
    
    /// Generate a meaningful name for a split file based on entity analysis
    pub fn generate_split_name(&self, base_name: &str, suffix: &str, entities: &[String], file_path: &Path) -> String {
        let extension = file_path.extension().and_then(|e| e.to_str()).unwrap_or("py");
        
        // Analyze entity names to suggest better suffixes
        let entity_analysis = self.analyze_entity_names(entities);
        
        let final_suffix = if !entity_analysis.is_empty() {
            entity_analysis
        } else {
            suffix.to_string()
        };
        
        format!("{}{}.{}", base_name, final_suffix, extension)
    }
    
    /// Analyze entity names to suggest appropriate suffixes
    pub fn analyze_entity_names(&self, entities: &[String]) -> String {
        let mut io_count = 0;
        let mut api_count = 0;
        let mut core_count = 0;
        let mut util_count = 0;
        
        for entity in entities {
            let lower_entity = entity.to_lowercase();
            
            if lower_entity.contains("read") || lower_entity.contains("write") ||
               lower_entity.contains("load") || lower_entity.contains("save") ||
               lower_entity.contains("file") || lower_entity.contains("io") {
                io_count += 1;
            } else if lower_entity.contains("api") || lower_entity.contains("endpoint") ||
                     lower_entity.contains("route") || lower_entity.contains("handler") ||
                     lower_entity.contains("controller") {
                api_count += 1;
            } else if lower_entity.contains("util") || lower_entity.contains("helper") ||
                     lower_entity.contains("tool") {
                util_count += 1;
            } else {
                core_count += 1;
            }
        }
        
        // Return the most appropriate suffix based on analysis
        if io_count > api_count && io_count > core_count && io_count > util_count {
            "_io".to_string()
        } else if api_count > core_count && api_count > util_count {
            "_api".to_string()
        } else if util_count > core_count {
            "_util".to_string()
        } else {
            "_core".to_string()
        }
    }

    /// Calculate value score for file splitting
    pub fn calculate_split_value(&self, loc: usize, _file_path: &Path) -> Result<SplitValue> {
        let size_factor = (loc as f64 / self.config.fsfile.huge_loc as f64).min(1.0);
        let cycle_factor = 0.0; // Placeholder - would check for participation in cycles
        let clone_factor = 0.0; // Placeholder - would check for clone mass
        
        let score = 0.6 * size_factor + 0.3 * cycle_factor + 0.1 * clone_factor;
        
        Ok(SplitValue { score })
    }

    /// Calculate effort required for file splitting
    pub fn calculate_split_effort(&self, _file_path: &Path) -> Result<SplitEffort> {
        // Placeholder - would analyze actual exports and external references
        Ok(SplitEffort {
            exports: 5,
            external_importers: 8,
        })
    }

    /// Extract Python entities (functions, classes)
    pub fn extract_python_entities(&self, content: &str) -> Result<Vec<EntityNode>> {
        let mut entities = Vec::new();
        let mut current_entity: Option<EntityNode> = None;
        let mut current_symbols = HashSet::new();
        let mut current_line_count = 0;
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Check for class or function definition
            if trimmed.starts_with("class ") {
                // Save previous entity
                if let Some(mut entity) = current_entity.take() {
                    entity.symbols = current_symbols.clone();
                    entity.loc = current_line_count;
                    entities.push(entity);
                }
                
                // Start new class entity
                if let Some(class_name) = trimmed.split_whitespace().nth(1) {
                    let clean_name = class_name.trim_end_matches(':').to_string();
                    current_entity = Some(EntityNode {
                        name: clean_name,
                        entity_type: "class".to_string(),
                        loc: 0,
                        symbols: HashSet::new(),
                    });
                    current_symbols = HashSet::new();
                    current_line_count = 0;
                }
            } else if trimmed.starts_with("def ") {
                // Save previous entity if it's not a method (methods stay with their class)
                if let Some(entity) = &current_entity {
                    if entity.entity_type == "function" {
                        if let Some(mut entity) = current_entity.take() {
                            entity.symbols = current_symbols.clone();
                            entity.loc = current_line_count;
                            entities.push(entity);
                        }
                        current_symbols = HashSet::new();
                        current_line_count = 0;
                    }
                } else {
                    // No current entity, so this is a top-level function
                    if let Some(func_name) = trimmed.split_whitespace().nth(1) {
                        let clean_name = func_name.split('(').next().unwrap_or(func_name).to_string();
                        current_entity = Some(EntityNode {
                            name: clean_name,
                            entity_type: "function".to_string(),
                            loc: 0,
                            symbols: HashSet::new(),
                        });
                        current_symbols = HashSet::new();
                        current_line_count = 0;
                    }
                }
            }
            
            // Extract symbols from current line
            if current_entity.is_some() && !trimmed.is_empty() {
                self.extract_symbols_from_line(trimmed, &mut current_symbols);
                current_line_count += 1;
            }
        }
        
        // Handle the last entity
        if let Some(mut entity) = current_entity {
            entity.symbols = current_symbols;
            entity.loc = current_line_count;
            entities.push(entity);
        }
        
        Ok(entities)
    }

    /// Extract JavaScript/TypeScript entities (functions, classes)
    pub fn extract_javascript_entities(&self, content: &str) -> Result<Vec<EntityNode>> {
        let mut entities = Vec::new();
        let mut current_entity: Option<EntityNode> = None;
        let mut current_symbols = HashSet::new();
        let mut current_line_count = 0;
        let mut brace_depth = 0;
        let mut in_entity = false;
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Count braces to track entity scope
            let open_braces = trimmed.chars().filter(|&c| c == '{').count();
            let close_braces = trimmed.chars().filter(|&c| c == '}').count();
            
            // Check for class or function definition
            if trimmed.starts_with("class ") || trimmed.contains("class ") {
                // Extract class name
                if let Some(class_start) = trimmed.find("class ") {
                    let after_class = &trimmed[class_start + 6..];
                    if let Some(class_name) = after_class.split_whitespace().next() {
                        let clean_name = class_name.trim_matches(['{', '(', ' ']).to_string();
                        
                        // Save previous entity
                        if let Some(mut entity) = current_entity.take() {
                            entity.symbols = current_symbols.clone();
                            entity.loc = current_line_count;
                            entities.push(entity);
                        }
                        
                        current_entity = Some(EntityNode {
                            name: clean_name,
                            entity_type: "class".to_string(),
                            loc: 0,
                            symbols: HashSet::new(),
                        });
                        current_symbols = HashSet::new();
                        current_line_count = 0;
                        brace_depth = 0;
                        in_entity = true;
                    }
                }
            } else if trimmed.starts_with("function ") || 
                     trimmed.contains(" function") || 
                     (trimmed.contains("const ") && trimmed.contains(" = ")) ||
                     (trimmed.contains("let ") && trimmed.contains(" = ")) {
                
                // Extract function name
                let mut func_name = String::new();
                
                if trimmed.starts_with("function ") {
                    if let Some(name) = trimmed.split_whitespace().nth(1) {
                        func_name = name.split('(').next().unwrap_or(name).to_string();
                    }
                } else if trimmed.contains(" = ") {
                    if let Some(equal_pos) = trimmed.find(" = ") {
                        let before_equal = &trimmed[..equal_pos];
                        if let Some(name) = before_equal.split_whitespace().last() {
                            func_name = name.to_string();
                        }
                    }
                }
                
                if !func_name.is_empty() && current_entity.is_none() {
                    // Top-level function
                    current_entity = Some(EntityNode {
                        name: func_name,
                        entity_type: "function".to_string(),
                        loc: 0,
                        symbols: HashSet::new(),
                    });
                    current_symbols = HashSet::new();
                    current_line_count = 0;
                    brace_depth = 0;
                    in_entity = true;
                }
            }
            
            if in_entity {
                brace_depth = (brace_depth + open_braces).saturating_sub(close_braces);
                
                // Extract symbols from current line
                if !trimmed.is_empty() {
                    self.extract_symbols_from_line(trimmed, &mut current_symbols);
                    current_line_count += 1;
                }
                
                // End of entity when braces balance (for functions) or class ends
                if brace_depth == 0 && open_braces > 0 {
                    if let Some(mut entity) = current_entity.take() {
                        entity.symbols = current_symbols.clone();
                        entity.loc = current_line_count;
                        entities.push(entity);
                    }
                    current_symbols = HashSet::new();
                    current_line_count = 0;
                    in_entity = false;
                }
            }
        }
        
        // Handle the last entity
        if let Some(mut entity) = current_entity {
            entity.symbols = current_symbols;
            entity.loc = current_line_count;
            entities.push(entity);
        }
        
        Ok(entities)
    }

    /// Extract Rust entities (functions, structs, impls)
    pub fn extract_rust_entities(&self, content: &str) -> Result<Vec<EntityNode>> {
        let mut entities = Vec::new();
        let mut current_entity: Option<EntityNode> = None;
        let mut current_symbols = HashSet::new();
        let mut current_line_count = 0;
        let mut brace_depth = 0;
        let mut in_entity = false;
        
        for line in content.lines() {
            let trimmed = line.trim();
            
            // Skip comments
            if trimmed.starts_with("//") {
                continue;
            }
            
            // Count braces to track scope
            let open_braces = trimmed.chars().filter(|&c| c == '{').count();
            let close_braces = trimmed.chars().filter(|&c| c == '}').count();
            
            // Check for struct, enum, fn, or impl
            if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
                if let Some(name) = trimmed.split_whitespace().nth(if trimmed.starts_with("pub") { 2 } else { 1 }) {
                    let clean_name = name.trim_matches(['{', '<', ' ']).to_string();
                    
                    // Save previous entity
                    if let Some(mut entity) = current_entity.take() {
                        entity.symbols = current_symbols.clone();
                        entity.loc = current_line_count;
                        entities.push(entity);
                    }
                    
                    current_entity = Some(EntityNode {
                        name: clean_name,
                        entity_type: "struct".to_string(),
                        loc: 0,
                        symbols: HashSet::new(),
                    });
                    current_symbols = HashSet::new();
                    current_line_count = 0;
                    brace_depth = 0;
                    in_entity = true;
                }
            } else if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") {
                if let Some(name) = trimmed.split_whitespace().nth(if trimmed.starts_with("pub") { 2 } else { 1 }) {
                    let clean_name = name.split('(').next().unwrap_or(name).to_string();
                    
                    // Save previous entity
                    if let Some(mut entity) = current_entity.take() {
                        entity.symbols = current_symbols.clone();
                        entity.loc = current_line_count;
                        entities.push(entity);
                    }
                    
                    current_entity = Some(EntityNode {
                        name: clean_name,
                        entity_type: "function".to_string(),
                        loc: 0,
                        symbols: HashSet::new(),
                    });
                    current_symbols = HashSet::new();
                    current_line_count = 0;
                    brace_depth = 0;
                    in_entity = true;
                }
            } else if trimmed.starts_with("impl ") {
                if let Some(impl_part) = trimmed.strip_prefix("impl ") {
                    let impl_name = impl_part.split_whitespace().next().unwrap_or("Impl").to_string();
                    
                    // Save previous entity
                    if let Some(mut entity) = current_entity.take() {
                        entity.symbols = current_symbols.clone();
                        entity.loc = current_line_count;
                        entities.push(entity);
                    }
                    
                    current_entity = Some(EntityNode {
                        name: format!("impl_{}", impl_name),
                        entity_type: "impl".to_string(),
                        loc: 0,
                        symbols: HashSet::new(),
                    });
                    current_symbols = HashSet::new();
                    current_line_count = 0;
                    brace_depth = 0;
                    in_entity = true;
                }
            }
            
            if in_entity {
                brace_depth = (brace_depth + open_braces).saturating_sub(close_braces);
                
                // Extract symbols from current line
                if !trimmed.is_empty() {
                    self.extract_symbols_from_line(trimmed, &mut current_symbols);
                    current_line_count += 1;
                }
                
                // End of entity when braces balance
                if brace_depth == 0 && (open_braces > 0 || close_braces > 0) {
                    if let Some(mut entity) = current_entity.take() {
                        entity.symbols = current_symbols.clone();
                        entity.loc = current_line_count;
                        entities.push(entity);
                    }
                    current_symbols = HashSet::new();
                    current_line_count = 0;
                    in_entity = false;
                }
            }
        }
        
        // Handle the last entity
        if let Some(mut entity) = current_entity {
            entity.symbols = current_symbols;
            entity.loc = current_line_count;
            entities.push(entity);
        }
        
        Ok(entities)
    }
    
    /// Extract symbols (identifiers) from a line of code
    pub fn extract_symbols_from_line(&self, line: &str, symbols: &mut HashSet<String>) {
        // Simple regex-like approach to extract identifiers
        let words: Vec<&str> = line.split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|word| !word.is_empty() && word.len() > 1) // Reduced from 2 to 1
            .filter(|word| !word.chars().all(|c| c.is_ascii_digit()))
            .collect();
        
        for word in words {
            // Filter out common keywords but allow certain important ones like 'self'
            if !Self::is_keyword(word) || word == "self" {
                symbols.insert(word.to_string());
            }
        }
    }
    
    /// Check if a word is a programming language keyword
    pub fn is_keyword(word: &str) -> bool {
        matches!(word, 
            "def" | "class" | "function" | "var" | "let" | "const" | "if" | "else" | "for" | 
            "while" | "return" | "import" | "from" | "fn" | "struct" | "enum" | "impl" | 
            "pub" | "use" | "mod" | "true" | "false" | "null" | "undefined" | "this" | "self" |
            "and" | "or" | "not" | "in" | "is" | "as" | "with" | "try" | "except" | "finally"
        )
    }
    
    /// Calculate Jaccard similarity between two sets of symbols
    pub fn calculate_jaccard_similarity(&self, set_a: &HashSet<String>, set_b: &HashSet<String>) -> f64 {
        if set_a.is_empty() && set_b.is_empty() {
            return 1.0;
        }
        
        let intersection_size = set_a.intersection(set_b).count();
        let union_size = set_a.union(set_b).count();
        
        if union_size == 0 {
            0.0
        } else {
            intersection_size as f64 / union_size as f64
        }
    }

    /// Extract imports from source file
    pub fn extract_imports(&self, file_path: &Path) -> Result<Vec<ImportStatement>> {
        let content = FileReader::read_to_string(file_path)?;
        let extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");
        
        match extension {
            "py" => self.extract_python_imports(&content),
            "js" | "jsx" | "ts" | "tsx" => self.extract_javascript_imports(&content),
            "rs" => self.extract_rust_imports(&content),
            _ => Ok(Vec::new()),
        }
    }

    /// Extract Python import statements
    pub fn extract_python_imports(&self, content: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();
        
        for (line_number, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            
            if let Some(import_part) = trimmed.strip_prefix("import ") {
                // Handle: import module
                let module = import_part.split_whitespace().next().unwrap_or("").to_string();
                imports.push(ImportStatement {
                    module,
                    imports: None,
                    import_type: "module".to_string(),
                    line_number: line_number + 1,
                });
            } else if let Some(from_part) = trimmed.strip_prefix("from ") {
                // Handle: from module import ...
                if let Some(import_pos) = from_part.find(" import ") {
                    let module = from_part[..import_pos].trim().to_string();
                    let import_list = from_part[import_pos + 8..].trim();
                    
                    let specific_imports = if import_list == "*" {
                        None // Star import
                    } else {
                        Some(import_list.split(',')
                            .map(|s| s.trim().to_string())
                            .collect())
                    };
                    
                    imports.push(ImportStatement {
                        module,
                        imports: specific_imports,
                        import_type: if import_list == "*" { "star" } else { "named" }.to_string(),
                        line_number: line_number + 1,
                    });
                }
            }
        }
        
        Ok(imports)
    }

    /// Extract JavaScript/TypeScript import statements  
    pub fn extract_javascript_imports(&self, content: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();
        
        for (line_number, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("/*") {
                continue;
            }
            
            if let Some(import_part) = trimmed.strip_prefix("import ") {
                // Handle various import patterns
                if let Some(from_pos) = import_part.find(" from ") {
                    let import_spec = import_part[..from_pos].trim();
                    let module_part = import_part[from_pos + 6..].trim().trim_matches(['"', '\'', ';']);
                    
                    let specific_imports = if import_spec.starts_with('*') {
                        None // Star import
                    } else if import_spec.starts_with('{') && import_spec.ends_with('}') {
                        // Named imports: { a, b, c }
                        let inner = &import_spec[1..import_spec.len()-1];
                        Some(inner.split(',')
                            .map(|s| s.trim().to_string())
                            .collect())
                    } else {
                        // Default import
                        Some(vec![import_spec.to_string()])
                    };
                    
                    imports.push(ImportStatement {
                        module: module_part.to_string(),
                        imports: specific_imports,
                        import_type: if import_spec.starts_with('*') { "star" } else { "named" }.to_string(),
                        line_number: line_number + 1,
                    });
                }
            }
        }
        
        Ok(imports)
    }

    /// Extract Rust use statements
    pub fn extract_rust_imports(&self, content: &str) -> Result<Vec<ImportStatement>> {
        let mut imports = Vec::new();
        
        for (line_number, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            
            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with("//") {
                continue;
            }
            
            if let Some(use_part) = trimmed.strip_prefix("use ") {
                let use_part = use_part.trim_end_matches(';');
                
                if let Some(brace_pos) = use_part.find('{') {
                    // Handle: use module::{item1, item2}
                    let module = use_part[..brace_pos].trim().to_string();
                    let items_part = &use_part[brace_pos + 1..];
                    
                    if let Some(close_brace) = items_part.find('}') {
                        let items = &items_part[..close_brace];
                        let specific_imports = Some(items.split(',')
                            .map(|s| s.trim().to_string())
                            .collect());
                        
                        imports.push(ImportStatement {
                            module,
                            imports: specific_imports,
                            import_type: "named".to_string(),
                            line_number: line_number + 1,
                        });
                    }
                } else {
                    // Handle: use module::item
                    imports.push(ImportStatement {
                        module: use_part.to_string(),
                        imports: None,
                        import_type: "module".to_string(),
                        line_number: line_number + 1,
                    });
                }
            }
        }
        
        Ok(imports)
    }

    /// Resolve import statement to local file path
    pub fn resolve_import_to_local_file(&self, import: &ImportStatement, dir_path: &Path) -> Option<PathBuf> {
        // This is a simplified resolution - in practice would be more sophisticated
        let module_name = &import.module;
        
        // Check if it's a relative import within the same directory
        if module_name.starts_with('.') {
            return None; // Skip relative imports for now
        }
        
        // Try common file extensions
        let extensions = ["py", "js", "ts", "jsx", "tsx", "rs"];
        
        for ext in &extensions {
            let potential_path = dir_path.join(format!("{}.{}", module_name, ext));
            if potential_path.exists() {
                return Some(potential_path);
            }
        }
        
        None
    }

    /// Discover large files to analyze
    pub async fn discover_large_files(&self, root_path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        self.collect_large_files_recursive(root_path, &mut files)?;
        Ok(files)
    }

    /// Recursively collect large files
    fn collect_large_files_recursive(&self, path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if self.should_skip_directory(path) {
            return Ok(());
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let child_path = entry.path();
            
            if child_path.is_dir() {
                self.collect_large_files_recursive(&child_path, files)?;
            } else if child_path.is_file() {
                if let Some(ext) = child_path.extension().and_then(|e| e.to_str()) {
                    if self.is_code_file(ext) {
                        let metadata = std::fs::metadata(&child_path)?;
                        let size_bytes = metadata.len() as usize;
                        
                        if size_bytes >= self.config.fsfile.huge_bytes {
                            files.push(child_path);
                        } else {
                            // Also check LOC for smaller files that might still be huge by line count
                            let loc = self.count_lines_of_code(&child_path)?;
                            if loc >= self.config.fsfile.huge_loc {
                                files.push(child_path);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }

    /// Check if directory should be skipped
    fn should_skip_directory(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        
        // Skip common generated/build/dependency directories
        path_str.contains("node_modules") ||
        path_str.contains("__pycache__") ||
        path_str.contains("target") ||
        path_str.contains(".git") ||
        path_str.contains("build") ||
        path_str.contains("dist")
    }
}