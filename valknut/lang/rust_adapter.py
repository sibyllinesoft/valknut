"""
Rust language adapter using tree-sitter.
"""

import logging
from pathlib import Path
from typing import Dict, List, Optional, Set

try:
    import tree_sitter_rust as ts_rust
    from tree_sitter import Language, Node, Parser
except ImportError:
    ts_rust = None
    Language = None
    Node = None
    Parser = None

import networkx as nx

from valknut.lang.common_ast import (
    BaseLanguageAdapter,
    Entity,
    EntityKind,
    ParseIndex,
    SourceLocation,
)

logger = logging.getLogger(__name__)


class RustAdapter(BaseLanguageAdapter):
    """Language adapter for Rust code analysis."""
    
    def __init__(self) -> None:
        super().__init__()
        if ts_rust is None:
            logger.warning("tree-sitter-rust not available")
            self._language = None
            self._parser = None
        else:
            self._language = Language(ts_rust.language())
            self._parser = Parser(self._language)
    
    @property
    def language(self) -> str:
        return "rust"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".rs"}
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover Rust files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["rust"])
    
    def parse_index(self, files: List[Path]) -> ParseIndex:
        """Parse Rust files and build index."""
        if self._parser is None:
            logger.warning("Rust parser not available")
            return ParseIndex({}, {}, nx.DiGraph(), nx.DiGraph())
        
        entities: Dict[str, Entity] = {}
        file_mapping: Dict[Path, str] = {}
        
        for file_path in files:
            try:
                file_entities = self._parse_file(file_path)
                
                # Add file entity
                file_entity_id = self._make_entity_id(file_path)
                file_entity = Entity(
                    id=file_entity_id,
                    name=file_path.name,
                    kind=EntityKind.FILE,
                    location=SourceLocation(file_path, 1, 1, 0, 0),
                    language=self.language,
                    raw_text=self._read_file(file_path),
                )
                
                entities[file_entity_id] = file_entity
                file_mapping[file_path] = file_entity_id
                
                # Add parsed entities
                for entity in file_entities:
                    entities[entity.id] = entity
                    
                    # Set parent-child relationships
                    if entity.parent_id:
                        if entity.parent_id in entities:
                            entities[entity.parent_id].children.append(entity.id)
                    else:
                        # Top-level entity, parent is file
                        entity.parent_id = file_entity_id
                        file_entity.children.append(entity.id)
                
            except Exception as e:
                logger.warning(f"Failed to parse {file_path}: {e}")
        
        # Build graphs
        import_graph = self._build_import_graph(entities)
        call_graph = self._build_call_graph(entities)
        
        return ParseIndex(entities, file_mapping, import_graph, call_graph)
    
    def _parse_file(self, file_path: Path) -> List[Entity]:
        """Parse a single Rust file."""
        if self._parser is None:
            return []
        
        try:
            content = self._read_file(file_path)
            if not content:
                return []
            
            tree = self._parser.parse(bytes(content, "utf8"))
            entities: List[Entity] = []
            
            self._extract_entities(tree.root_node, file_path, content, entities)
            
            return entities
        
        except Exception as e:
            logger.warning(f"Failed to parse {file_path}: {e}")
            return []
    
    def _extract_entities(self, node: "Node", file_path: Path, content: str, entities: List[Entity], parent_id: Optional[str] = None) -> None:
        """Extract entities from tree-sitter node."""
        # Map tree-sitter node types to our entity types
        node_type_map = {
            "struct_item": EntityKind.STRUCT,
            "enum_item": EntityKind.ENUM,
            "trait_item": EntityKind.TRAIT,
            "impl_item": EntityKind.CLASS,  # Rust impl blocks as class-like
            "function_item": EntityKind.FUNCTION,
            "function_signature_item": EntityKind.FUNCTION,
            "mod_item": EntityKind.MODULE,
            "const_item": EntityKind.VARIABLE,
            "static_item": EntityKind.VARIABLE,
            "let_declaration": EntityKind.VARIABLE,
        }
        
        if node.type in node_type_map:
            entity_kind = node_type_map[node.type]
            entity_name = self._extract_name(node)
            
            if entity_name:
                entity_id = self._make_entity_id(file_path, entity_name)
                
                # Get source location
                start_point = node.start_point
                end_point = node.end_point
                
                location = SourceLocation(
                    file_path=file_path,
                    start_line=start_point.row + 1,
                    end_line=end_point.row + 1,
                    start_column=start_point.column,
                    end_column=end_point.column,
                )
                
                # Extract raw text
                raw_text = content[node.start_byte:node.end_byte] if node.start_byte < len(content) else ""
                
                # Extract parameters for functions
                parameters = self._extract_parameters(node)
                
                entity = Entity(
                    id=entity_id,
                    name=entity_name,
                    kind=entity_kind,
                    location=location,
                    language=self.language,
                    parent_id=parent_id,
                    parameters=parameters,
                    raw_text=raw_text,
                )
                
                entities.append(entity)
                parent_id = entity_id  # Children will have this as parent
        
        # Recursively process child nodes
        for child in node.children:
            self._extract_entities(child, file_path, content, entities, parent_id)
    
    def _extract_name(self, node: "Node") -> Optional[str]:
        """Extract name from a node."""
        # Look for identifier in common patterns
        for child in node.children:
            if child.type == "identifier":
                return child.text.decode("utf8")
            elif child.type == "type_identifier":
                return child.text.decode("utf8")
        
        # For impl blocks, we might have type_identifier deeper
        if node.type == "impl_item":
            for child in node.children:
                if child.type in ["generic_type", "type_identifier"]:
                    return self._extract_type_name(child)
        
        return None
    
    def _extract_type_name(self, node: "Node") -> Optional[str]:
        """Extract type name from type node."""
        if node.type == "type_identifier":
            return node.text.decode("utf8")
        
        # For generic types like Vec<T>
        for child in node.children:
            if child.type == "type_identifier":
                return child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node: "Node") -> List[str]:
        """Extract parameter names from function node."""
        parameters = []
        
        # Find parameters node
        for child in node.children:
            if child.type == "parameters":
                for param_node in child.children:
                    if param_node.type == "parameter":
                        # Find pattern (parameter name)
                        for param_child in param_node.children:
                            if param_child.type == "identifier":
                                parameters.append(param_child.text.decode("utf8"))
                                break
                            elif param_child.type in ["mut_pattern", "reference_pattern"]:
                                # Handle &mut self, &self, etc.
                                name = self._extract_pattern_name(param_child)
                                if name:
                                    parameters.append(name)
                                    break
        
        return parameters
    
    def _extract_pattern_name(self, node: "Node") -> Optional[str]:
        """Extract name from pattern node."""
        for child in node.children:
            if child.type == "identifier":
                return child.text.decode("utf8")
            elif child.type in ["mut_pattern", "reference_pattern"]:
                return self._extract_pattern_name(child)
        return None
    
    def _build_import_graph(self, entities: Dict[str, Entity]) -> nx.DiGraph:
        """Build import dependency graph."""
        graph = nx.DiGraph()
        
        # Add all entities as nodes
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                graph.add_node(entity_id, **entity.__dict__)
        
        # Parse use statements
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE and entity.raw_text:
                imports = self._extract_imports(entity.raw_text)
                for imported_module in imports:
                    # Try to resolve import to actual file
                    imported_entity = self._resolve_import(imported_module, entity.location.file_path, entities)
                    if imported_entity:
                        graph.add_edge(entity_id, imported_entity)
        
        return graph
    
    def _extract_imports(self, content: str) -> List[str]:
        """Extract use statements from content."""
        imports = []
        
        # Simple regex-based extraction for Rust use statements
        import re
        
        # Match use statements
        use_patterns = [
            r"use\s+([^;]+);",  # use module::path;
        ]
        
        for pattern in use_patterns:
            matches = re.findall(pattern, content)
            imports.extend(matches)
        
        return imports
    
    def _resolve_import(self, use_path: str, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve use path to entity ID."""
        # Simplified resolution for Rust modules
        # In practice, would need to handle Cargo.toml, lib.rs, mod.rs, etc.
        
        # Handle relative imports
        if use_path.startswith("crate::") or use_path.startswith("super::") or use_path.startswith("self::"):
            # This would need proper Rust module resolution
            pass
        
        # For now, just try to find files with matching names
        parts = use_path.split("::")
        if len(parts) > 0:
            module_name = parts[-1]
            potential_file = current_file.parent / f"{module_name}.rs"
            
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == potential_file:
                    return entity_id
        
        return None
    
    def _build_call_graph(self, entities: Dict[str, Entity]) -> nx.DiGraph:
        """Build call dependency graph."""
        graph = nx.DiGraph()
        
        # Add function entities as nodes
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FUNCTION:
                graph.add_node(entity_id, **entity.__dict__)
        
        # For Rust, we'd need to parse function calls
        # This is a simplified placeholder
        return graph


# Register the adapter
def register_rust_adapter() -> None:
    """Register Rust adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(RustAdapter)