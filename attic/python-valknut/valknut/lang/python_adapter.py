"""
Python language adapter using tree-sitter-python.
"""

import ast
import logging
from pathlib import Path
from typing import Dict, Generator, List, Optional, Set
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


class PythonAdapter(TreeSitterBaseAdapter):
    """Language adapter for Python code analysis using tree-sitter."""
    
    @property
    def language(self) -> str:
        return "python"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".py", ".pyi"}
    
    @property
    def parser_module(self) -> str:
        return "tree_sitter_python"
    
    @property
    def parser_language_function(self) -> str:
        return "language"
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover Python files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["python"])
    
    def _setup_mappings(self) -> None:
        """Setup node type mappings and import patterns for Python."""
        # Map tree-sitter node types to our entity types
        self._node_type_map = {
            "class_definition": EntityKind.CLASS,
            "function_definition": EntityKind.FUNCTION,
            "async_function_definition": EntityKind.FUNCTION,
            # Variables are handled differently in Python
        }
        
        # Regex patterns for extracting import statements
        self._import_patterns = [
            r"^import\s+([^\s#]+)",  # import module
            r"^from\s+([^\s#]+)\s+import",  # from module import ...
        ]
    
    def _extract_name(self, node) -> Optional[str]:
        """Extract name from a Python tree-sitter node."""
        # Look for identifier in Python-specific patterns
        for child in node.children:
            if child.type == "identifier":
                return child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node) -> List[str]:
        """Extract parameter names from Python function node."""
        parameters = []
        
        # Find parameters node in Python function definition
        for child in node.children:
            if child.type == "parameters":
                self._extract_python_parameters_from_container(child, parameters)
        
        return parameters
    
    def _extract_python_parameters_from_container(self, container, parameters: List[str]) -> None:
        """Extract parameters from Python parameters container."""
        for param_node in container.children:
            if param_node.type == "identifier":
                # Simple parameter
                parameters.append(param_node.text.decode("utf8"))
            elif param_node.type == "default_parameter":
                # Parameter with default value
                for child in param_node.children:
                    if child.type == "identifier":
                        parameters.append(child.text.decode("utf8"))
                        break
            elif param_node.type == "typed_parameter":
                # Type-annotated parameter
                for child in param_node.children:
                    if child.type == "identifier":
                        parameters.append(child.text.decode("utf8"))
                        break
            elif param_node.type == "typed_default_parameter":
                # Type-annotated parameter with default
                for child in param_node.children:
                    if child.type == "identifier":
                        parameters.append(child.text.decode("utf8"))
                        break
    
    def _extract_docstring(self, node, content: str) -> Optional[str]:
        """Extract Python docstring from function/class node."""
        # Look for string literal as first child of the body
        for child in node.children:
            if child.type == "block":
                # First statement in block might be a docstring
                for stmt in child.children:
                    if stmt.type == "expression_statement":
                        for expr_child in stmt.children:
                            if expr_child.type == "string":
                                # Extract the string content, removing quotes
                                docstring = expr_child.text.decode("utf8")
                                # Remove triple quotes or single quotes
                                if docstring.startswith('"""') and docstring.endswith('"""'):
                                    return docstring[3:-3].strip()
                                elif docstring.startswith("'''") and docstring.endswith("'''"):
                                    return docstring[3:-3].strip()
                                elif docstring.startswith('"') and docstring.endswith('"'):
                                    return docstring[1:-1].strip()
                                elif docstring.startswith("'") and docstring.endswith("'"):
                                    return docstring[1:-1].strip()
                                return docstring.strip()
                        # Only check first statement
                        break
                break
        
        return None
    
    def _create_parsed_import_from_regex(self, match: re.Match, file_path: Path, content: str) -> ParsedImport:
        """Create ParsedImport from regex match for Python."""
        module_path = match.group(1)
        is_relative = module_path.startswith(".")
        
        return ParsedImport(
            module=module_path,
            is_relative=is_relative
        )
    
    def _resolve_relative_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Python relative import."""
        module_path = parsed_import.module.lstrip(".")
        level = len(parsed_import.module) - len(module_path)  # Number of leading dots
        
        # Calculate the base directory based on relative level
        base_dir = current_file.parent
        for _ in range(level - 1):  # -1 because one level is the current directory
            base_dir = base_dir.parent
        
        potential_paths = []
        
        if module_path:
            # Convert module path to file path
            module_file_path = module_path.replace(".", "/")
            potential_paths.extend([
                base_dir / f"{module_file_path}.py",
                base_dir / module_file_path / "__init__.py",
            ])
        else:
            # Import from current package (__init__.py)
            potential_paths.append(base_dir / "__init__.py")
        
        for potential_path in potential_paths:
            resolved_path = potential_path.resolve()
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == resolved_path:
                    return entity_id
        
        return None
    
    def _resolve_absolute_import(self, parsed_import: ParsedImport, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve Python absolute import."""
        module_parts = parsed_import.module.split(".")
        
        # Try different combinations to find the file
        for i in range(len(module_parts), 0, -1):
            partial_module = "/".join(module_parts[:i])
            remaining_parts = module_parts[i:]
            
            # Try as file or package
            potential_names = [
                f"{partial_module}.py",
                f"{partial_module}/__init__.py",
            ]
            
            if remaining_parts:
                # If there are remaining parts, try them as nested modules
                nested_path = "/".join(remaining_parts)
                potential_names.extend([
                    f"{partial_module}/{nested_path}.py",
                    f"{partial_module}/{nested_path}/__init__.py",
                ])
            
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE:
                    file_path_str = str(entity.location.file_path)
                    for potential_name in potential_names:
                        if file_path_str.endswith(potential_name):
                            return entity_id
        
        return None
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """Extract Python-specific type features."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        # Count type annotations using AST as fallback
        try:
            tree = ast.parse(source)
            type_analyzer = TypeAnalyzer()
            type_analyzer.visit(tree)
            
            total_annotations = type_analyzer.total_annotations
            any_types = type_analyzer.any_types
            casts = type_analyzer.casts
            
            typed_coverage = total_annotations / max(1, type_analyzer.total_items)
            any_ratio = any_types / max(1, total_annotations)
            casts_per_kloc = (casts / max(1, entity.loc)) * 1000
            
            return {
                "typed_coverage_ratio": min(1.0, typed_coverage),
                "any_ratio": min(1.0, any_ratio),
                "casts_per_kloc": min(100.0, casts_per_kloc),
            }
            
        except Exception as e:
            logger.debug(f"Type analysis failed for {entity.id}: {e}")
            return {
                "typed_coverage_ratio": 0.0,
                "any_ratio": 0.0,
                "casts_per_kloc": 0.0,
            }
    
    def exception_features(self, entity: Entity) -> Dict[str, float]:
        """Extract exception-related features."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        try:
            tree = ast.parse(source)
            exception_analyzer = ExceptionAnalyzer()
            exception_analyzer.visit(tree)
            
            raises = exception_analyzer.raises
            exception_types = len(exception_analyzer.exception_types)
            
            exception_density = (raises / max(1, entity.loc)) * 1000
            exception_variety = (exception_types / max(1, entity.loc)) * 1000
            
            return {
                "exception_density": min(100.0, exception_density),
                "exception_variety": min(50.0, exception_variety),
            }
            
        except Exception as e:
            logger.debug(f"Exception analysis failed for {entity.id}: {e}")
            return {
                "exception_density": 0.0,
                "exception_variety": 0.0,
            }
    
    def cohesion_features(self, entity: Entity) -> Dict[str, float]:
        """Extract cohesion-related features."""
        if entity.kind != EntityKind.CLASS:
            return {"lcom_like": 0.0}
        
        if not entity.raw_text:
            return {"lcom_like": 0.0}
        
        try:
            tree = ast.parse(entity.raw_text)
            cohesion_analyzer = CohesionAnalyzer()
            cohesion_analyzer.visit(tree)
            
            methods = cohesion_analyzer.methods
            fields = cohesion_analyzer.fields
            method_field_usage = cohesion_analyzer.method_field_usage
            
            if not methods or not fields:
                return {"lcom_like": 0.0}
            
            # Calculate LCOM-like metric
            shared_fields = 0
            total_pairs = len(methods) * (len(methods) - 1) // 2
            
            if total_pairs > 0:
                for i, method1 in enumerate(methods):
                    for method2 in methods[i+1:]:
                        fields1 = method_field_usage.get(method1, set())
                        fields2 = method_field_usage.get(method2, set())
                        if fields1 & fields2:  # Shared fields
                            shared_fields += 1
                
                lcom_like = 1.0 - (shared_fields / total_pairs)
            else:
                lcom_like = 0.0
            
            return {"lcom_like": min(1.0, max(0.0, lcom_like))}
            
        except Exception as e:
            logger.debug(f"Cohesion analysis failed for {entity.id}: {e}")
            return {"lcom_like": 0.0}


# Keep AST-based analyzers for detailed Python analysis
class TypeAnalyzer(ast.NodeVisitor):
    """Analyze Python type annotations."""
    
    def __init__(self) -> None:
        self.total_annotations = 0
        self.any_types = 0
        self.casts = 0
        self.total_items = 0
    
    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        """Visit function definition."""
        self.total_items += 1
        
        # Check return type annotation
        if node.returns:
            self.total_annotations += 1
            if self._is_any_type(node.returns):
                self.any_types += 1
        
        # Check parameter annotations
        for arg in node.args.args:
            self.total_items += 1
            if arg.annotation:
                self.total_annotations += 1
                if self._is_any_type(arg.annotation):
                    self.any_types += 1
        
        self.generic_visit(node)
    
    def visit_Call(self, node: ast.Call) -> None:
        """Visit function call."""
        # Check for cast calls
        if (isinstance(node.func, ast.Name) and 
            node.func.id in {'cast'}) or \
           (isinstance(node.func, ast.Attribute) and 
            node.func.attr in {'cast'}):
            self.casts += 1
        
        self.generic_visit(node)
    
    def _is_any_type(self, annotation: ast.AST) -> bool:
        """Check if annotation is Any type."""
        if isinstance(annotation, ast.Name):
            return annotation.id == 'Any'
        elif isinstance(annotation, ast.Attribute):
            return annotation.attr == 'Any'
        return False


class ExceptionAnalyzer(ast.NodeVisitor):
    """Analyze Python exception usage."""
    
    def __init__(self) -> None:
        self.raises = 0
        self.exception_types: Set[str] = set()
    
    def visit_Raise(self, node: ast.Raise) -> None:
        """Visit raise statement."""
        self.raises += 1
        
        if node.exc:
            exc_type = self._get_exception_type(node.exc)
            if exc_type:
                self.exception_types.add(exc_type)
        
        self.generic_visit(node)
    
    def _get_exception_type(self, node: ast.AST) -> Optional[str]:
        """Extract exception type name."""
        if isinstance(node, ast.Name):
            return node.id
        elif isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
            return node.func.id
        return None


class CohesionAnalyzer(ast.NodeVisitor):
    """Analyze class cohesion."""
    
    def __init__(self) -> None:
        self.methods: List[str] = []
        self.fields: Set[str] = set()
        self.method_field_usage: Dict[str, Set[str]] = {}
        self.current_method: Optional[str] = None
    
    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        """Visit class definition."""
        for item in node.body:
            if isinstance(item, ast.FunctionDef):
                self.methods.append(item.name)
            elif isinstance(item, ast.Assign):
                for target in item.targets:
                    if isinstance(target, ast.Name):
                        self.fields.add(target.id)
        
        self.generic_visit(node)
    
    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        """Visit method definition."""
        self.current_method = node.name
        self.method_field_usage[node.name] = set()
        self.generic_visit(node)
        self.current_method = None
    
    def visit_Attribute(self, node: ast.Attribute) -> None:
        """Visit attribute access."""
        if (self.current_method and 
            isinstance(node.value, ast.Name) and 
            node.value.id == 'self'):
            self.method_field_usage[self.current_method].add(node.attr)
        
        self.generic_visit(node)


# Register the adapter
def register_python_adapter() -> None:
    """Register Python adapter with the global registry."""
    from valknut.core.registry import register_language_adapter
    register_language_adapter(PythonAdapter)