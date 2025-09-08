"""
Go language adapter using tree-sitter.
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


class GoAdapter(TreeSitterBaseAdapter):
    """Language adapter for Go code analysis using tree-sitter."""
    
    @property
    def language(self) -> str:
        return "go"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".go"}
    
    @property
    def parser_module(self) -> str:
        return "tree_sitter_go"
    
    @property
    def parser_language_function(self) -> str:
        return "language"
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover Go files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["go"])
    
    def _setup_mappings(self) -> None:
        """Setup node type mappings and import patterns for Go."""
        # Map tree-sitter node types to our entity types
        self._node_type_map = {
            "type_declaration": EntityKind.STRUCT,  # Go type declarations can be structs, interfaces, etc.
            "struct_type": EntityKind.STRUCT,
            "interface_type": EntityKind.INTERFACE,
            "function_declaration": EntityKind.FUNCTION,
            "method_declaration": EntityKind.METHOD,
            "var_declaration": EntityKind.VARIABLE,
            "const_declaration": EntityKind.VARIABLE,
        }
        
        # Regex patterns for extracting import statements
        self._import_patterns = [
            r'import\s+"([^"]+)"',  # import "package"
            r'import\s+\w+\s+"([^"]+)"',  # import alias "package"
            r'import\s*\(\s*"([^"]+)"',  # import ( "package" )
        ]
    
    def _extract_name(self, node) -> Optional[str]:
        """Extract name from a Go tree-sitter node."""
        # Go uses different node types for names
        for child in node.children:
            if child.type == "identifier":
                return child.text.decode("utf8")
            elif child.type == "type_identifier":
                return child.text.decode("utf8")
            elif child.type == "field_identifier":
                return child.text.decode("utf8")
        
        # Special handling for type declarations
        if node.type == "type_declaration":
            # Look for type_spec -> type_identifier
            for child in node.children:
                if child.type == "type_spec":
                    for spec_child in child.children:
                        if spec_child.type == "type_identifier":
                            return spec_child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node) -> List[str]:
        """Extract parameter names from Go function/method node."""
        parameters = []
        
        # Find parameter_list node
        for child in node.children:
            if child.type == "parameter_list":
                self._extract_go_parameters_from_list(child, parameters)
        
        return parameters
    
    def _extract_go_parameters_from_list(self, param_list, parameters: List[str]) -> None:
        """Extract parameters from Go parameter list."""
        for child in param_list.children:
            if child.type == "parameter_declaration":
                # Go parameters can be: name type, name1, name2 type, etc.
                identifiers = []
                for param_child in child.children:
                    if param_child.type == "identifier":
                        identifiers.append(param_child.text.decode("utf8"))
                
                # Add all identifiers as parameters
                parameters.extend(identifiers)
    
    def _create_parsed_import_from_regex(self, match: re.Match, file_path: Path, content: str) -> ParsedImport:
        """Create ParsedImport from regex match for Go."""
        import_path = match.group(1)
        
        # Go imports are generally absolute from GOPATH/module root
        # Relative imports are rare and not recommended
        is_relative = "/" not in import_path  # Very simple heuristic
        
        return ParsedImport(
            module=import_path,
            is_relative=is_relative
        )
    
    def _resolve_relative_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Go relative import (rare in Go)."""
        # Go doesn't typically use relative imports
        # If we encounter one, try simple name matching
        import_name = parsed_import.module
        
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                file_dir_name = entity.location.file_path.parent.name
                if file_dir_name == import_name:
                    return entity_id
        
        return None
    
    def _resolve_absolute_import(self, parsed_import: ParsedImport, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Go absolute import."""
        import_path = parsed_import.module
        
        # Skip standard library packages
        std_packages = ["fmt", "os", "io", "net", "http", "encoding", "time", "strings", "strconv", "context"]
        if any(import_path.startswith(pkg) for pkg in std_packages):
            return None
        
        # For project-local imports, try to match package paths
        parts = import_path.split("/")
        
        # Try to find files in directories matching the import path
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                entity_parts = entity.location.file_path.parts
                
                # Check if the entity path contains the import path components
                if len(parts) <= len(entity_parts):
                    # Simple containment check
                    if all(part in entity_parts for part in parts[-2:]):  # Check last 2 parts
                        return entity_id
        
        return None
    
    def _find_go_module_root(self, file_path: Path) -> Optional[Path]:
        """Find the Go module root directory."""
        current_dir = file_path.parent
        
        while current_dir != current_dir.parent:  # Not reached filesystem root
            # Check for go.mod (definitive module root)
            if (current_dir / "go.mod").exists():
                return current_dir
            
            current_dir = current_dir.parent
        
        return None
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """Extract Go-specific type features."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            # Count Go-specific features
            interface_methods = len(re.findall(r'func\s+\([^)]*\)\s+\w+', source))  # Methods
            struct_fields = len(re.findall(r'^\s*\w+\s+\w+', source, re.MULTILINE))  # Struct fields
            type_assertions = len(re.findall(r'\.\(\w+\)', source))  # Type assertions
            type_switches = len(re.findall(r'switch\s+.*\.\(type\)', source))
            
            lines = max(1, len(source.splitlines()))
            
            # Calculate ratios
            method_density = (interface_methods / lines) * 1000
            field_density = (struct_fields / lines) * 1000
            type_assertion_density = (type_assertions / lines) * 1000
            type_switch_density = (type_switches / lines) * 1000
            
            return {
                "method_density": min(100.0, method_density),
                "field_density": min(200.0, field_density),
                "type_assertion_density": min(50.0, type_assertion_density),
                "type_switch_density": min(10.0, type_switch_density),
            }
            
        except Exception as e:
            logger.debug(f"Type analysis failed for {entity.id}: {e}")
            return {
                "method_density": 0.0,
                "field_density": 0.0,
                "type_assertion_density": 0.0,
                "type_switch_density": 0.0,
            }
    
    def exception_features(self, entity: Entity) -> Dict[str, float]:
        """Extract error handling features for Go."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            # Count Go error handling patterns
            if_err_checks = len(re.findall(r'if\s+err\s*!=\s*nil', source))
            error_returns = len(re.findall(r'return\s+.*error', source))
            panic_calls = len(re.findall(r'\bpanic\(', source))
            recover_calls = len(re.findall(r'\brecover\(\)', source))
            
            lines = max(1, len(source.splitlines()))
            
            # Calculate ratios
            error_check_density = (if_err_checks / lines) * 1000
            error_return_density = (error_returns / lines) * 1000
            panic_density = (panic_calls / lines) * 1000
            recover_ratio = (recover_calls / max(1, panic_calls)) if panic_calls > 0 else 0
            
            return {
                "error_check_density": min(100.0, error_check_density),
                "error_return_density": min(100.0, error_return_density),
                "panic_density": min(10.0, panic_density),
                "recover_ratio": min(1.0, recover_ratio),
            }
            
        except Exception as e:
            logger.debug(f"Exception analysis failed for {entity.id}: {e}")
            return {
                "error_check_density": 0.0,
                "error_return_density": 0.0,
                "panic_density": 0.0,
                "recover_ratio": 0.0,
            }
    
    def cohesion_features(self, entity: Entity) -> Dict[str, float]:
        """Extract cohesion-related features for Go."""
        if entity.kind not in {EntityKind.STRUCT, EntityKind.INTERFACE}:
            return {"lcom_like": 0.0}
        
        if not entity.raw_text:
            return {"lcom_like": 0.0}
        
        try:
            # For Go, we'll look at method receiver relationships
            source = entity.raw_text
            
            # Find methods that use the same receiver type
            receiver_methods = re.findall(r'func\s+\([^)]*(\w+)[^)]*\)\s+(\w+)', source)
            method_names = [method[1] for method in receiver_methods]
            
            # Find field accesses within methods (simplified)
            field_accesses = re.findall(r'(\w+)\.(\w+)', source)
            
            if not method_names:
                return {"lcom_like": 0.0}
            
            # Simple cohesion metric based on shared field access patterns
            # This is a simplified implementation
            unique_fields = set(access[1] for access in field_accesses)
            
            if len(unique_fields) == 0:
                return {"lcom_like": 0.0}
            
            # Calculate a basic cohesion metric
            field_method_ratio = len(unique_fields) / len(method_names)
            lcom_like = min(1.0, field_method_ratio)
            
            return {"lcom_like": lcom_like}
            
        except Exception as e:
            logger.debug(f"Cohesion analysis failed for {entity.id}: {e}")
            return {"lcom_like": 0.0}


# Register the adapter
def register_go_adapter() -> None:
    """Register Go adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(GoAdapter)