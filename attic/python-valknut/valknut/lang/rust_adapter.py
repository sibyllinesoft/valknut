"""
Rust language adapter using tree-sitter.
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


class RustAdapter(TreeSitterBaseAdapter):
    """Language adapter for Rust code analysis using tree-sitter."""
    
    @property
    def language(self) -> str:
        return "rust"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".rs"}
    
    @property
    def parser_module(self) -> str:
        return "tree_sitter_rust"
    
    @property
    def parser_language_function(self) -> str:
        return "language"
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover Rust files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["rust"])
    
    def _setup_mappings(self) -> None:
        """Setup node type mappings and import patterns for Rust."""
        # Map tree-sitter node types to our entity types
        self._node_type_map = {
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
        
        # Regex patterns for extracting use statements
        self._import_patterns = [
            r"use\s+([^;]+);",  # use module::path;
        ]
    
    def _extract_name(self, node) -> Optional[str]:
        """Extract name from a Rust tree-sitter node."""
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
    
    def _extract_type_name(self, node) -> Optional[str]:
        """Extract type name from type node."""
        if node.type == "type_identifier":
            return node.text.decode("utf8")
        
        # For generic types like Vec<T>
        for child in node.children:
            if child.type == "type_identifier":
                return child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node) -> List[str]:
        """Extract parameter names from Rust function node."""
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
    
    def _extract_pattern_name(self, node) -> Optional[str]:
        """Extract name from pattern node."""
        for child in node.children:
            if child.type == "identifier":
                return child.text.decode("utf8")
            elif child.type in ["mut_pattern", "reference_pattern"]:
                return self._extract_pattern_name(child)
        return None
    
    def _create_parsed_import_from_regex(self, match: re.Match, file_path: Path, content: str) -> ParsedImport:
        """Create ParsedImport from regex match for Rust."""
        use_statement = match.group(1).strip()
        
        # Parse Rust use statements which can be complex:
        # use std::collections::HashMap;
        # use crate::module;
        # use super::sibling;
        # use self::child;
        
        is_relative = use_statement.startswith(("crate::", "super::", "self::"))
        
        return ParsedImport(
            module=use_statement,
            is_relative=is_relative
        )
    
    def _resolve_relative_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Rust relative import."""
        use_path = parsed_import.module
        
        if use_path.startswith("crate::"):
            # Absolute path from crate root
            return self._resolve_crate_import(use_path, current_file, entities)
        elif use_path.startswith("super::"):
            # Parent module
            return self._resolve_super_import(use_path, current_file, entities)
        elif use_path.startswith("self::"):
            # Current module
            return self._resolve_self_import(use_path, current_file, entities)
        
        return None
    
    def _resolve_crate_import(self, use_path: str, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve crate:: import from project root."""
        # Find the crate root (directory containing Cargo.toml or lib.rs/main.rs)
        crate_root = self._find_crate_root(current_file)
        if not crate_root:
            return None
        
        # Remove crate:: prefix
        module_path = use_path[7:]  # len("crate::")
        
        return self._resolve_rust_module_path(module_path, crate_root, entities)
    
    def _resolve_super_import(self, use_path: str, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve super:: import."""
        # Count super:: prefixes
        super_count = 0
        remaining_path = use_path
        while remaining_path.startswith("super::"):
            super_count += 1
            remaining_path = remaining_path[7:]  # Remove "super::"
        
        # Navigate up the directory tree
        base_dir = current_file.parent
        for _ in range(super_count):
            base_dir = base_dir.parent
        
        return self._resolve_rust_module_path(remaining_path, base_dir, entities)
    
    def _resolve_self_import(self, use_path: str, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve self:: import."""
        # Remove self:: prefix
        module_path = use_path[6:]  # len("self::")
        
        return self._resolve_rust_module_path(module_path, current_file.parent, entities)
    
    def _find_crate_root(self, file_path: Path) -> Optional[Path]:
        """Find the crate root directory."""
        current_dir = file_path.parent
        
        while current_dir != current_dir.parent:  # Not reached filesystem root
            # Check for Cargo.toml (definitive crate root)
            if (current_dir / "Cargo.toml").exists():
                return current_dir
            
            # Check for src/lib.rs or src/main.rs
            if (current_dir / "src" / "lib.rs").exists() or (current_dir / "src" / "main.rs").exists():
                return current_dir
            
            current_dir = current_dir.parent
        
        return None
    
    def _resolve_rust_module_path(self, module_path: str, base_dir: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve a Rust module path to a file."""
        if not module_path:
            return None
        
        parts = module_path.split("::")
        potential_paths = []
        
        # Build potential file paths
        current_path = base_dir
        for i, part in enumerate(parts):
            if i == len(parts) - 1:
                # Last part - could be a file or module
                potential_paths.extend([
                    current_path / f"{part}.rs",
                    current_path / part / "mod.rs",
                ])
            else:
                # Intermediate part - directory
                current_path = current_path / part
        
        # Try to find matching entities
        for potential_path in potential_paths:
            resolved_path = potential_path.resolve()
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == resolved_path:
                    return entity_id
        
        return None
    
    def _resolve_absolute_import(self, parsed_import: ParsedImport, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Rust absolute import (external crates)."""
        # Most absolute imports in Rust are external crates
        # We only try to resolve if it might be internal
        
        use_path = parsed_import.module
        
        # Skip known external crates
        external_crates = ["std", "alloc", "core"]
        first_part = use_path.split("::")[0]
        
        if first_part in external_crates:
            return None
        
        # For other cases, try simple name matching
        parts = use_path.split("::")
        last_part = parts[-1]
        
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                file_stem = entity.location.file_path.stem
                if file_stem == last_part or (file_stem == "mod" and entity.location.file_path.parent.name == last_part):
                    return entity_id
        
        return None
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """Extract Rust-specific type features."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            # Count Rust-specific features
            generic_params = len(re.findall(r'<[^>]+>', source))
            trait_bounds = len(re.findall(r':\s*\w+', source))  # Simple trait bounds
            lifetimes = len(re.findall(r"'[a-zA-Z_]\w*", source))
            unsafe_blocks = len(re.findall(r'\bunsafe\s*{', source))
            
            lines = max(1, len(source.splitlines()))
            
            # Calculate ratios
            generics_density = (generic_params / lines) * 1000
            trait_usage = (trait_bounds / lines) * 1000
            lifetime_density = (lifetimes / lines) * 1000
            unsafe_ratio = (unsafe_blocks / lines) * 1000
            
            return {
                "generics_density": min(100.0, generics_density),
                "trait_usage": min(100.0, trait_usage),
                "lifetime_density": min(50.0, lifetime_density),
                "unsafe_ratio": min(10.0, unsafe_ratio),
            }
            
        except Exception as e:
            logger.debug(f"Type analysis failed for {entity.id}: {e}")
            return {
                "generics_density": 0.0,
                "trait_usage": 0.0,
                "lifetime_density": 0.0,
                "unsafe_ratio": 0.0,
            }
    
    def exception_features(self, entity: Entity) -> Dict[str, float]:
        """Extract error handling features for Rust."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            # Count Rust error handling patterns
            result_types = len(re.findall(r'\bResult<[^>]+>', source))
            option_types = len(re.findall(r'\bOption<[^>]+>', source))
            unwrap_calls = len(re.findall(r'\.unwrap\(\)', source))
            expect_calls = len(re.findall(r'\.expect\(', source))
            panic_calls = len(re.findall(r'\bpanic!\(', source))
            
            lines = max(1, len(source.splitlines()))
            
            # Calculate ratios
            error_handling_density = ((result_types + option_types) / lines) * 1000
            unsafe_unwrap_ratio = (unwrap_calls / max(1, result_types + option_types))
            panic_density = (panic_calls / lines) * 1000
            
            return {
                "error_handling_density": min(100.0, error_handling_density),
                "unsafe_unwrap_ratio": min(1.0, unsafe_unwrap_ratio),
                "panic_density": min(10.0, panic_density),
            }
            
        except Exception as e:
            logger.debug(f"Exception analysis failed for {entity.id}: {e}")
            return {
                "error_handling_density": 0.0,
                "unsafe_unwrap_ratio": 0.0,
                "panic_density": 0.0,
            }


# Register the adapter
def register_rust_adapter() -> None:
    """Register Rust adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(RustAdapter)