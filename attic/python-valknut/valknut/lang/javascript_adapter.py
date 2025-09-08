"""
JavaScript language adapter using tree-sitter.
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


class JavaScriptAdapter(TreeSitterBaseAdapter):
    """Language adapter for JavaScript code analysis using tree-sitter."""
    
    @property
    def language(self) -> str:
        return "javascript"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".js", ".jsx", ".mjs", ".cjs"}
    
    @property
    def parser_module(self) -> str:
        return "tree_sitter_javascript"
    
    @property
    def parser_language_function(self) -> str:
        return "language"
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover JavaScript files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["javascript"])
    
    def _setup_mappings(self) -> None:
        """Setup node type mappings and import patterns for JavaScript."""
        # Map tree-sitter node types to our entity types
        self._node_type_map = {
            "class_declaration": EntityKind.CLASS,
            "function_declaration": EntityKind.FUNCTION,
            "method_definition": EntityKind.METHOD,
            "arrow_function": EntityKind.FUNCTION,
            "function_expression": EntityKind.FUNCTION,
            "variable_declaration": EntityKind.VARIABLE,
            "lexical_declaration": EntityKind.VARIABLE,  # let/const
        }
        
        # Regex patterns for extracting import statements
        self._import_patterns = [
            r"import\s+.*\s+from\s+['\"]([^'\"]+)['\"]",  # ES6 import ... from "..."
            r"import\s+['\"]([^'\"]+)['\"]",  # import "..."
            r"require\(['\"]([^'\"]+)['\"]\)",  # CommonJS require("...")
        ]
    
    def _extract_name(self, node) -> Optional[str]:
        """Extract name from a JavaScript tree-sitter node."""
        # Look for identifier in common patterns
        for child in node.children:
            if child.type == "identifier":
                return child.text.decode("utf8")
            elif child.type == "property_identifier":
                return child.text.decode("utf8")
            elif child.type == "variable_declarator":
                # For variable declarations, get the variable name
                for grandchild in child.children:
                    if grandchild.type == "identifier":
                        return grandchild.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node) -> List[str]:
        """Extract parameter names from JavaScript function/method node."""
        parameters = []
        
        # Find formal_parameters node
        for child in node.children:
            if child.type == "formal_parameters":
                for param_node in child.children:
                    if param_node.type == "identifier":
                        parameters.append(param_node.text.decode("utf8"))
                    elif param_node.type == "rest_pattern":
                        # Rest parameters (...args)
                        for rest_child in param_node.children:
                            if rest_child.type == "identifier":
                                parameters.append(f"...{rest_child.text.decode('utf8')}")
        
        return parameters
    
    def _create_parsed_import_from_regex(self, match: re.Match, file_path: Path, content: str) -> ParsedImport:
        """Create ParsedImport from regex match for JavaScript."""
        module_path = match.group(1)
        is_relative = module_path.startswith(".")
        
        return ParsedImport(
            module=module_path,
            is_relative=is_relative
        )
    
    def _resolve_relative_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve JavaScript relative import."""
        import_path = parsed_import.module.lstrip(".")
        
        # Try different extensions
        extensions = [".js", ".jsx", ".mjs", ".cjs"]
        potential_paths = []
        
        for ext in extensions:
            potential_paths.append(current_file.parent / f"{import_path}{ext}")
            # Also try index files
            potential_paths.append(current_file.parent / import_path / f"index{ext}")
        
        for potential_path in potential_paths:
            resolved_path = potential_path.resolve()
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == resolved_path:
                    return entity_id
        
        return None
    
    def _resolve_absolute_import(self, parsed_import: ParsedImport, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve JavaScript absolute import."""
        # For JavaScript, absolute imports could be:
        # 1. Node modules (ignore for now)
        # 2. Absolute paths from project root
        
        if parsed_import.module.startswith("/"):
            # Absolute path from root - rare in practice
            return None
        
        # Try to find files with matching names in the project
        module_parts = parsed_import.module.split("/")
        last_part = module_parts[-1]
        
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                file_stem = entity.location.file_path.stem
                if file_stem == last_part or file_stem == "index":
                    # Check if the path structure matches
                    entity_parts = entity.location.file_path.parts
                    if len(module_parts) <= len(entity_parts):
                        # Simple name matching for now
                        if any(part in entity_parts for part in module_parts):
                            return entity_id
        
        return None


# Register the adapter
def register_javascript_adapter() -> None:
    """Register JavaScript adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(JavaScriptAdapter)