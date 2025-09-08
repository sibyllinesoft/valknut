"""
TypeScript language adapter using tree-sitter.
"""

import logging
from pathlib import Path
from typing import Dict, List, Optional, Set
import re

import networkx as nx

from valknut.lang.common_ast import (
    TreeSitterBaseAdapter,
    Entity,
    EntityKind,
    ParseIndex,
    SourceLocation,
    ParsedImport,
)

logger = logging.getLogger(__name__)


class TypeScriptAdapter(TreeSitterBaseAdapter):
    """Language adapter for TypeScript code analysis using tree-sitter."""
    
    @property
    def language(self) -> str:
        return "typescript"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".ts", ".tsx", ".d.ts"}
    
    @property
    def parser_module(self) -> str:
        return "tree_sitter_typescript"
    
    @property
    def parser_language_function(self) -> str:
        return "language_typescript"
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover TypeScript files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["typescript"])
    
    def _setup_mappings(self) -> None:
        """Setup node type mappings and import patterns for TypeScript."""
        # Map tree-sitter node types to our entity types
        self._node_type_map = {
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
        
        # Regex patterns for extracting import statements
        self._import_patterns = [
            r"import\s+.*\s+from\s+['\"]([^'\"]+)['\"]",  # import ... from "..."
            r"import\s+['\"]([^'\"]+)['\"]",  # import "..."
        ]
    
    def _extract_name(self, node) -> Optional[str]:
        """Extract name from a TypeScript tree-sitter node."""
        # Look for identifier in common patterns
        for child in node.children:
            if child.type == "identifier":
                return child.text.decode("utf8")
            elif child.type == "property_identifier":
                return child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node) -> List[str]:
        """Extract parameter names from TypeScript function/method node."""
        parameters = []
        
        # Find formal_parameters node
        for child in node.children:
            if child.type == "formal_parameters":
                for param_node in child.children:
                    if param_node.type == "required_parameter" or param_node.type == "optional_parameter":
                        # Find identifier in parameter
                        for param_child in param_node.children:
                            if param_child.type == "identifier":
                                param_name = param_child.text.decode("utf8")
                                if param_node.type == "optional_parameter":
                                    param_name += "?"
                                parameters.append(param_name)
                                break
        
        return parameters
    
    def _create_parsed_import_from_regex(self, match: re.Match, file_path: Path, content: str) -> ParsedImport:
        """Create ParsedImport from regex match for TypeScript."""
        module_path = match.group(1)
        is_relative = module_path.startswith(".")
        
        return ParsedImport(
            module=module_path,
            is_relative=is_relative
        )
    
    def _resolve_relative_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve TypeScript relative import."""
        import_path = parsed_import.module.lstrip(".")
        
        # TypeScript extensions and resolution order
        extensions = [".ts", ".tsx", ".d.ts"]
        potential_paths = []
        
        # Try direct file matches first
        for ext in extensions:
            potential_paths.append(current_file.parent / f"{import_path}{ext}")
            
        # Try index files
        for ext in extensions:
            potential_paths.append(current_file.parent / import_path / f"index{ext}")
        
        # Try without extension (TypeScript can resolve these)
        if not any(import_path.endswith(ext) for ext in extensions):
            potential_paths.append(current_file.parent / f"{import_path}.js")  # JS files in mixed projects
        
        for potential_path in potential_paths:
            resolved_path = potential_path.resolve()
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == resolved_path:
                    return entity_id
        
        return None
    
    def _resolve_absolute_import(self, parsed_import: ParsedImport, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve TypeScript absolute import."""
        # TypeScript has similar module resolution to JavaScript
        # but with additional support for .d.ts files
        
        if parsed_import.module.startswith("@"):
            # Scoped packages - usually external, skip
            return None
        
        # Try to find files with matching names in the project
        module_parts = parsed_import.module.split("/")
        last_part = module_parts[-1]
        
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                file_stem = entity.location.file_path.stem
                
                # Direct name match
                if file_stem == last_part:
                    return entity_id
                
                # Index file match
                if file_stem == "index":
                    entity_parts = entity.location.file_path.parts
                    if len(module_parts) <= len(entity_parts):
                        # Check if path structure matches
                        if any(part in entity_parts for part in module_parts):
                            return entity_id
        
        return None
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """Extract TypeScript-specific type features."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        # Count TypeScript-specific features
        try:
            # Basic regex-based analysis for TypeScript features
            any_types = len(re.findall(r'\bany\b', source))
            type_annotations = len(re.findall(r':\s*\w+', source))
            interfaces = len(re.findall(r'\binterface\s+\w+', source))
            generics = len(re.findall(r'<[^>]+>', source))
            
            lines = len(source.splitlines())
            
            # Calculate ratios
            any_ratio = (any_types / max(1, type_annotations)) if type_annotations > 0 else 0
            type_density = (type_annotations / max(1, lines)) * 1000
            interface_density = (interfaces / max(1, lines)) * 1000
            generics_usage = (generics / max(1, lines)) * 1000
            
            return {
                "any_ratio": min(1.0, any_ratio),
                "type_density": min(100.0, type_density),
                "interface_density": min(50.0, interface_density),
                "generics_usage": min(50.0, generics_usage),
            }
            
        except Exception as e:
            logger.debug(f"Type analysis failed for {entity.id}: {e}")
            return {
                "any_ratio": 0.0,
                "type_density": 0.0,
                "interface_density": 0.0,
                "generics_usage": 0.0,
            }
    
    def exception_features(self, entity: Entity) -> Dict[str, float]:
        """Extract exception-related features for TypeScript."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            # Count error handling patterns
            try_blocks = len(re.findall(r'\btry\s*{', source))
            throw_statements = len(re.findall(r'\bthrow\s+', source))
            error_types = len(re.findall(r'\bError\b', source))
            
            lines = max(1, len(source.splitlines()))
            
            exception_density = (throw_statements / lines) * 1000
            error_handling_ratio = (try_blocks / max(1, throw_statements)) if throw_statements > 0 else 0
            
            return {
                "exception_density": min(100.0, exception_density),
                "error_handling_ratio": min(1.0, error_handling_ratio),
            }
            
        except Exception as e:
            logger.debug(f"Exception analysis failed for {entity.id}: {e}")
            return {
                "exception_density": 0.0,
                "error_handling_ratio": 0.0,
            }


# Register the adapter
def register_typescript_adapter() -> None:
    """Register TypeScript adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(TypeScriptAdapter)