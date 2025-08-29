"""
TypeScript language adapter using tree-sitter.
"""

import logging
from pathlib import Path
from typing import Dict, List, Optional, Set

try:
    import tree_sitter_typescript as ts_typescript
    from tree_sitter import Language, Node, Parser
except ImportError:
    ts_typescript = None
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


class TypeScriptAdapter(BaseLanguageAdapter):
    """Language adapter for TypeScript code analysis."""
    
    def __init__(self) -> None:
        super().__init__()
        if ts_typescript is None:
            logger.warning("tree-sitter-typescript not available")
            self._language = None
            self._parser = None
        else:
            self._language = Language(ts_typescript.language_typescript())
            self._parser = Parser(self._language)
    
    @property
    def language(self) -> str:
        return "typescript"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".ts", ".tsx", ".d.ts"}
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover TypeScript files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["typescript"])
    
    def parse_index(self, files: List[Path]) -> ParseIndex:
        """Parse TypeScript files and build index."""
        if self._parser is None:
            logger.warning("TypeScript parser not available")
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
        """Parse a single TypeScript file."""
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
            "class_declaration": EntityKind.CLASS,
            "interface_declaration": EntityKind.INTERFACE,
            "function_declaration": EntityKind.FUNCTION,
            "method_definition": EntityKind.METHOD,
            "arrow_function": EntityKind.FUNCTION,
            "function_expression": EntityKind.FUNCTION,
            "variable_declaration": EntityKind.VARIABLE,
            "enum_declaration": EntityKind.ENUM,
            "type_alias_declaration": EntityKind.INTERFACE,
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
                
                # Extract parameters for functions/methods
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
            elif child.type == "property_identifier":
                return child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node: "Node") -> List[str]:
        """Extract parameter names from function/method node."""
        parameters = []
        
        # Find formal_parameters node
        for child in node.children:
            if child.type == "formal_parameters":
                for param_node in child.children:
                    if param_node.type == "required_parameter" or param_node.type == "optional_parameter":
                        # Find identifier in parameter
                        for param_child in param_node.children:
                            if param_child.type == "identifier":
                                parameters.append(param_child.text.decode("utf8"))
                                break
        
        return parameters
    
    def _build_import_graph(self, entities: Dict[str, Entity]) -> nx.DiGraph:
        """Build import dependency graph."""
        graph = nx.DiGraph()
        
        # Add all entities as nodes
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                graph.add_node(entity_id, **entity.__dict__)
        
        # For TypeScript, we'd need to parse import statements
        # This is a simplified version
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE and entity.raw_text:
                imports = self._extract_imports(entity.raw_text)
                for imported_file in imports:
                    # Try to resolve import to actual file
                    imported_entity = self._resolve_import(imported_file, entity.location.file_path, entities)
                    if imported_entity:
                        graph.add_edge(entity_id, imported_entity)
        
        return graph
    
    def _extract_imports(self, content: str) -> List[str]:
        """Extract import statements from content."""
        imports = []
        
        # Simple regex-based extraction (could be improved with tree-sitter)
        import re
        
        # Match import statements
        import_patterns = [
            r"import\s+.*\s+from\s+['\"]([^'\"]+)['\"]",  # import ... from "..."
            r"import\s+['\"]([^'\"]+)['\"]",  # import "..."
        ]
        
        for pattern in import_patterns:
            matches = re.findall(pattern, content)
            imports.extend(matches)
        
        return imports
    
    def _resolve_import(self, import_path: str, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve import path to entity ID."""
        # Simplified resolution - in practice would need more sophisticated logic
        if import_path.startswith("."):
            # Relative import
            resolved_path = current_file.parent / f"{import_path}.ts"
            resolved_path = resolved_path.resolve()
            
            # Find entity with matching path
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == resolved_path:
                    return entity_id
        
        return None
    
    def _build_call_graph(self, entities: Dict[str, Entity]) -> nx.DiGraph:
        """Build call dependency graph."""
        graph = nx.DiGraph()
        
        # Add function/method entities as nodes
        for entity_id, entity in entities.items():
            if entity.kind in {EntityKind.FUNCTION, EntityKind.METHOD}:
                graph.add_node(entity_id, **entity.__dict__)
        
        # For TypeScript, we'd need to parse function calls
        # This is a simplified placeholder
        return graph


# Register the adapter
def register_typescript_adapter() -> None:
    """Register TypeScript adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(TypeScriptAdapter)