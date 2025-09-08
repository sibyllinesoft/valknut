"""
Bash language adapter using tree-sitter.
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


class BashAdapter(TreeSitterBaseAdapter):
    """Language adapter for Bash/Shell script analysis using tree-sitter."""
    
    @property
    def language(self) -> str:
        return "bash"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".sh", ".bash", ".zsh"}
    
    @property
    def parser_module(self) -> str:
        return "tree_sitter_bash"
    
    @property
    def parser_language_function(self) -> str:
        return "language"
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover Bash files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["bash"])
    
    def _setup_mappings(self) -> None:
        """Setup node type mappings and import patterns for Bash."""
        # Map tree-sitter node types to our entity types
        # Bash doesn't have traditional classes/functions, but we can identify:
        self._node_type_map = {
            "function_definition": EntityKind.FUNCTION,
            "variable_assignment": EntityKind.VARIABLE,
            # Bash scripts don't have classes, but we can treat large script blocks as functions
        }
        
        # Regex patterns for extracting source/import statements
        self._import_patterns = [
            r'source\s+["\']?([^"\';\s]+)["\']?',  # source filename
            r'\.\s+["\']?([^"\';\s]+)["\']?',      # . filename (same as source)
        ]
    
    def _extract_name(self, node) -> Optional[str]:
        """Extract name from a Bash tree-sitter node."""
        # Look for word (identifier equivalent in bash)
        for child in node.children:
            if child.type == "word":
                return child.text.decode("utf8")
            elif child.type == "variable_name":
                return child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node) -> List[str]:
        """Extract parameter names from Bash function node."""
        parameters = []
        
        # Bash functions don't have formal parameters, but we can look for $1, $2, etc. usage
        # This is a simplified approach - in practice, parameters are accessed via $1, $2, etc.
        
        # We'll just return common parameter patterns if found in the function body
        if hasattr(node, 'text'):
            text = node.text.decode("utf8")
            # Find positional parameters
            param_matches = re.findall(r'\$(\d+)', text)
            if param_matches:
                parameters = [f"${num}" for num in sorted(set(param_matches))]
        
        return parameters
    
    def _create_parsed_import_from_regex(self, match: re.Match, file_path: Path, content: str) -> ParsedImport:
        """Create ParsedImport from regex match for Bash."""
        source_path = match.group(1)
        
        # Bash sources are typically relative to the current script
        is_relative = not source_path.startswith("/")
        
        return ParsedImport(
            module=source_path,
            is_relative=is_relative
        )
    
    def _resolve_relative_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Bash relative source."""
        source_path = parsed_import.module
        
        # Try different potential paths
        potential_paths = []
        
        # Direct relative path
        potential_paths.append(current_file.parent / source_path)
        
        # Add .sh extension if not present
        if not source_path.endswith(('.sh', '.bash', '.zsh')):
            for ext in ['.sh', '.bash', '.zsh']:
                potential_paths.append(current_file.parent / f"{source_path}{ext}")
        
        for potential_path in potential_paths:
            resolved_path = potential_path.resolve()
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == resolved_path:
                    return entity_id
        
        return None
    
    def _resolve_absolute_import(self, parsed_import: ParsedImport, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Bash absolute source."""
        source_path = parsed_import.module
        
        # For absolute paths, try to find exact matches
        absolute_path = Path(source_path)
        
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE and entity.location.file_path == absolute_path.resolve():
                return entity_id
        
        return None
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """Extract Bash-specific features."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            # Count Bash-specific features
            variable_assignments = len(re.findall(r'^\s*\w+=', source, re.MULTILINE))
            command_substitutions = len(re.findall(r'\$\(.*?\)', source))
            pipe_operations = len(re.findall(r'\|', source))
            conditional_blocks = len(re.findall(r'\bif\b|\bcase\b|\bwhile\b|\bfor\b', source))
            
            lines = max(1, len(source.splitlines()))
            
            # Calculate ratios
            variable_density = (variable_assignments / lines) * 1000
            command_sub_density = (command_substitutions / lines) * 1000
            pipe_density = (pipe_operations / lines) * 1000
            control_flow_density = (conditional_blocks / lines) * 1000
            
            return {
                "variable_density": min(200.0, variable_density),
                "command_sub_density": min(100.0, command_sub_density),
                "pipe_density": min(100.0, pipe_density),
                "control_flow_density": min(100.0, control_flow_density),
            }
            
        except Exception as e:
            logger.debug(f"Type analysis failed for {entity.id}: {e}")
            return {
                "variable_density": 0.0,
                "command_sub_density": 0.0,
                "pipe_density": 0.0,
                "control_flow_density": 0.0,
            }
    
    def exception_features(self, entity: Entity) -> Dict[str, float]:
        """Extract error handling features for Bash."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            # Count Bash error handling patterns
            exit_calls = len(re.findall(r'\bexit\s+\d+', source))
            set_errexit = len(re.findall(r'set\s+-e', source))  # Exit on error
            error_checks = len(re.findall(r'\$\?\s*[!=]=\s*0', source))  # Check exit status
            trap_statements = len(re.findall(r'\btrap\b', source))
            
            lines = max(1, len(source.splitlines()))
            
            # Calculate ratios
            exit_density = (exit_calls / lines) * 1000
            error_check_density = (error_checks / lines) * 1000
            trap_density = (trap_statements / lines) * 1000
            safety_ratio = 1.0 if set_errexit > 0 else 0.0
            
            return {
                "exit_density": min(50.0, exit_density),
                "error_check_density": min(100.0, error_check_density),
                "trap_density": min(20.0, trap_density),
                "safety_ratio": safety_ratio,
            }
            
        except Exception as e:
            logger.debug(f"Exception analysis failed for {entity.id}: {e}")
            return {
                "exit_density": 0.0,
                "error_check_density": 0.0,
                "trap_density": 0.0,
                "safety_ratio": 0.0,
            }
    
    def cohesion_features(self, entity: Entity) -> Dict[str, float]:
        """Extract cohesion-related features for Bash (limited applicability)."""
        # Bash scripts don't have traditional cohesion metrics
        # We'll provide a simple metric based on function usage patterns
        
        if entity.kind != EntityKind.FUNCTION:
            return {"lcom_like": 0.0}
        
        if not entity.raw_text:
            return {"lcom_like": 0.0}
        
        try:
            source = entity.raw_text
            
            # Count variable references and function calls within the script/function
            variable_refs = len(re.findall(r'\$\w+|\$\{\w+\}', source))
            function_calls = len(re.findall(r'\b\w+\s*\(\)', source))
            
            # Simple cohesion metric based on internal references
            total_refs = variable_refs + function_calls
            lines = max(1, len(source.splitlines()))
            
            # Higher reference density suggests more cohesive code
            reference_density = (total_refs / lines) * 1000
            lcom_like = min(1.0, reference_density / 100.0)  # Normalize to 0-1
            
            return {"lcom_like": lcom_like}
            
        except Exception as e:
            logger.debug(f"Cohesion analysis failed for {entity.id}: {e}")
            return {"lcom_like": 0.0}


# Register the adapter
def register_bash_adapter() -> None:
    """Register Bash adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(BashAdapter)