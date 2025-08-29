"""
Python language adapter using libcst and basic AST analysis.
"""

import ast
import logging
from pathlib import Path
from typing import Dict, Generator, List, Optional, Set

import libcst as cst
import networkx as nx

from valknut.lang.common_ast import (
    BaseLanguageAdapter,
    Entity,
    EntityKind,
    ParseIndex,
    SourceLocation,
)

logger = logging.getLogger(__name__)


class PythonAdapter(BaseLanguageAdapter):
    """Language adapter for Python code analysis."""
    
    @property
    def language(self) -> str:
        return "python"
    
    @property
    def file_extensions(self) -> Set[str]:
        return {".py", ".pyi"}
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """Discover Python files."""
        from valknut.io.fsrepo import FileDiscovery
        
        discovery = FileDiscovery()
        return discovery.discover(roots, include_patterns, exclude_patterns, ["python"])
    
    def parse_index(self, files: List[Path]) -> ParseIndex:
        """Parse Python files and build index."""
        print(f"DEBUG: parse_index called with {len(files)} files")
        entities: Dict[str, Entity] = {}
        file_mapping: Dict[Path, str] = {}
        
        for file_path in files:
            print(f"DEBUG: Processing file {file_path}")
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
        
        return ParseIndex(
            entities=entities,
            files=file_mapping,
            import_graph=import_graph,
        )
    
    def entities(self, index: ParseIndex) -> Generator[Entity, None, None]:
        """Generate entities for analysis."""
        for entity in index.entities.values():
            yield entity
    
    def call_graph(self, index: ParseIndex) -> Optional[nx.DiGraph]:
        """Build call graph (simplified implementation)."""
        # For now, return None - would need more sophisticated analysis
        return None
    
    def import_graph(self, index: ParseIndex) -> nx.DiGraph:
        """Build import graph."""
        return index.import_graph or nx.DiGraph()
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """Extract Python-specific type features."""
        if not entity.raw_text:
            return {}
        
        source = entity.raw_text
        
        # Count type annotations
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
    
    def _parse_file(self, file_path: Path) -> List[Entity]:
        """Parse a single Python file."""
        entities = []
        
        try:
            print(f"DEBUG: Parsing file {file_path}")
            source_code = self._read_file(file_path)
            print(f"DEBUG: Source code length: {len(source_code)} chars")
            
            # Use AST first to get line number information, then enhance with CST
            ast_tree = ast.parse(source_code)
            source_lines = source_code.splitlines()
            print(f"DEBUG: Source has {len(source_lines)} lines")
            
            # Extract entities using AST visitor (which has line numbers)
            visitor = PythonASTEntityExtractor(file_path, self.language, source_lines)
            visitor.visit(ast_tree)
            
            entities = visitor.entities
            print(f"DEBUG: Extracted {len(entities)} entities")
            for entity in entities:
                print(f"DEBUG: Entity {entity.id}, raw_text={'present' if entity.raw_text else 'MISSING'}")
                if entity.raw_text:
                    print(f"DEBUG: First 50 chars: {repr(entity.raw_text[:50])}")
            
        except Exception as e:
            print(f"DEBUG: Exception in _parse_file: {e}")
            logger.warning(f"Failed to parse {file_path}: {e}")
        
        return entities
    
    def _build_import_graph(self, entities: Dict[str, Entity]) -> nx.DiGraph:
        """Build import dependency graph."""
        graph = nx.DiGraph()
        
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                graph.add_node(entity_id, path=str(entity.location.file_path))
                
                # Parse imports from file content
                imports = self._extract_imports(entity.raw_text or "")
                
                for imported_module in imports:
                    # Find corresponding entity (simplified)
                    imported_entity_id = self._resolve_import(imported_module, entities)
                    if imported_entity_id and imported_entity_id != entity_id:
                        graph.add_edge(entity_id, imported_entity_id)
        
        return graph
    
    def _extract_imports(self, source_code: str) -> List[str]:
        """Extract import statements from source code."""
        imports = []
        
        try:
            tree = ast.parse(source_code)
            
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    for alias in node.names:
                        imports.append(alias.name)
                elif isinstance(node, ast.ImportFrom):
                    if node.module:
                        imports.append(node.module)
        
        except Exception:
            pass
        
        return imports
    
    def _resolve_import(self, import_name: str, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve import to entity ID (simplified)."""
        # Simple resolution - in practice would need more sophisticated module resolution
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                module_name = entity.location.file_path.stem
                if module_name == import_name or import_name.endswith(module_name):
                    return entity_id
        return None


class PythonEntityExtractor(cst.CSTVisitor):
    """CST visitor to extract Python entities."""
    
    def __init__(self, file_path: Path, language: str) -> None:
        self.file_path = file_path
        self.language = language
        self.entities: List[Entity] = []
        self.class_stack: List[str] = []
        self.source_lines = None  # Will be set when we have the source
        
    def set_source_lines(self, source_code: str) -> None:
        """Set source code lines for line number calculation."""
        self.source_lines = source_code.splitlines()
    
    def visit_ClassDef(self, node: cst.ClassDef) -> Optional[bool]:
        """Visit class definition."""
        class_name = node.name.value
        
        # Calculate actual location from CST node
        start_line = getattr(node, 'lineno', 1) if hasattr(node, 'lineno') else 1
        # Estimate end line by finding the end of the class definition
        end_line = self._estimate_end_line(node) if self.source_lines else start_line + 10
        location = SourceLocation(self.file_path, start_line, end_line, 0, 0)
        
        # Build qualified name
        qualified_name = ".".join(self.class_stack + [class_name])
        entity_id = f"python://{self.file_path}::{qualified_name}"
        
        # Extract source text for this class
        raw_text = self._extract_source_text(start_line, end_line)
        
        # Extract fields and methods (simplified)
        fields = []
        
        entity = Entity(
            id=entity_id,
            name=class_name,
            kind=EntityKind.CLASS,
            location=location,
            language=self.language,
            fields=fields,
            raw_text=raw_text,
        )
        
        self.entities.append(entity)
        self.class_stack.append(class_name)
        
        return True
    
    def leave_ClassDef(self, original_node: cst.ClassDef) -> None:
        """Leave class definition."""
        if self.class_stack:
            self.class_stack.pop()
    
    def visit_FunctionDef(self, node: cst.FunctionDef) -> Optional[bool]:
        """Visit function definition."""
        func_name = node.name.value
        
        # Calculate actual location from CST node
        start_line = getattr(node, 'lineno', 1) if hasattr(node, 'lineno') else 1
        # Estimate end line by finding the end of the function definition
        end_line = self._estimate_end_line(node) if self.source_lines else start_line + 10
        location = SourceLocation(self.file_path, start_line, end_line, 0, 0)
        
        # Build qualified name
        if self.class_stack:
            qualified_name = ".".join(self.class_stack + [func_name])
            kind = EntityKind.METHOD
        else:
            qualified_name = func_name
            kind = EntityKind.FUNCTION
        
        entity_id = f"python://{self.file_path}::{qualified_name}"
        
        # Extract parameters (simplified)
        parameters = []
        if node.params:
            for param in node.params.params:
                parameters.append(param.name.value)
        
        # Extract source text for this function
        raw_text = self._extract_source_text(start_line, end_line)
        
        entity = Entity(
            id=entity_id,
            name=func_name,
            kind=kind,
            location=location,
            language=self.language,
            parameters=parameters,
            raw_text=raw_text,
        )
        
        self.entities.append(entity)
        
        return False  # Don't visit nested functions
    
    def _find_end_line(self, node: ast.AST) -> int:
        """Find the end line of an AST node by analyzing indentation."""
        start_line = node.lineno
        
        # If the node has an end_lineno attribute (Python 3.8+), use it
        if hasattr(node, 'end_lineno') and node.end_lineno:
            return node.end_lineno
        
        # Otherwise, estimate by finding the next same-level construct
        if start_line > len(self.source_lines):
            return start_line
            
        # Get the indentation of the definition line
        def_line = self.source_lines[start_line - 1]  # Convert to 0-based
        base_indent = len(def_line) - len(def_line.lstrip())
        
        # Look for the end by finding next construct at same or lower indentation
        end_line = len(self.source_lines)  # Default to end of file
        
        for i in range(start_line, len(self.source_lines)):
            line = self.source_lines[i]
            if line.strip():  # Skip empty lines
                line_indent = len(line) - len(line.lstrip())
                # If we find a line at same or lower indentation level, that's likely the end
                if line_indent <= base_indent:
                    end_line = i  # 0-based, so this is the line before
                    break
        
        return end_line
    
    def _extract_source_text(self, start_line: int, end_line: int) -> Optional[str]:
        """Extract source text between line numbers (1-based)."""
        if not self.source_lines:
            return None
        
        # Convert to 0-based indexing
        start_idx = max(0, start_line - 1)
        end_idx = min(len(self.source_lines), end_line)
        
        if start_idx >= end_idx:
            return None
        
        return "\n".join(self.source_lines[start_idx:end_idx])


class PythonASTEntityExtractor(ast.NodeVisitor):
    """Extract entities from Python AST with source text."""
    
    def __init__(self, file_path: Path, language: str, source_lines: List[str]):
        self.file_path = file_path
        self.language = language
        self.source_lines = source_lines
        self.entities: List[Entity] = []
        self.current_class: Optional[str] = None
    
    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        """Visit function definition."""
        self._create_function_entity(node)
        self.generic_visit(node)
    
    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        """Visit async function definition."""
        self._create_function_entity(node)
        self.generic_visit(node)
    
    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        """Visit class definition."""
        old_class = self.current_class
        self.current_class = node.name
        
        # Create class entity
        entity_id = self._make_entity_id(node.name)
        start_line = node.lineno
        end_line = self._find_end_line(node.lineno)
        source_text = self._extract_source_text(start_line, end_line)
        
        entity = Entity(
            id=entity_id,
            name=node.name,
            kind=EntityKind.CLASS,
            location=SourceLocation(
                self.file_path, 
                start_line, 
                getattr(node, 'col_offset', 0) + 1,
                end_line,
                0
            ),
            language=self.language,
            raw_text=source_text,
            parameters=[],
        )
        
        self.entities.append(entity)
        self.generic_visit(node)
        self.current_class = old_class
    
    def _create_function_entity(self, node) -> None:
        """Create entity for function or method."""
        name = node.name
        if self.current_class:
            entity_id = f"python://{self.file_path}::{self.current_class}.{name}"
            kind = EntityKind.METHOD
        else:
            entity_id = f"python://{self.file_path}::{name}"
            kind = EntityKind.FUNCTION
        
        # Find function end line
        start_line = node.lineno
        end_line = self._find_end_line(node.lineno)
        
        # Extract source text for this function
        source_text = self._extract_source_text(start_line, end_line)
        
        # Extract parameters
        parameters = []
        for arg in node.args.args:
            parameters.append(arg.arg)
        
        entity = Entity(
            id=entity_id,
            name=name,
            kind=kind,
            location=SourceLocation(
                self.file_path, 
                start_line, 
                getattr(node, 'col_offset', 0) + 1,
                end_line,
                0
            ),
            language=self.language,
            raw_text=source_text,  # THIS IS THE KEY FIX!
            parameters=parameters,
        )
        
        self.entities.append(entity)
    
    def _make_entity_id(self, name: str) -> str:
        """Make entity ID."""
        if self.current_class:
            return f"python://{self.file_path}::{self.current_class}.{name}"
        else:
            return f"python://{self.file_path}::{name}"
    
    def _find_end_line(self, start_line: int) -> int:
        """Find end line of a code block by indentation."""
        if not self.source_lines or start_line > len(self.source_lines):
            return start_line
        
        # Find the indentation level of the definition line
        def_line = self.source_lines[start_line - 1]  # Convert to 0-based
        base_indent = len(def_line) - len(def_line.lstrip())
        
        # Look for the end of the block
        end_line = len(self.source_lines)  # Default to end of file
        
        for i in range(start_line, len(self.source_lines)):  # start_line is 1-based, convert to 0-based
            line = self.source_lines[i]
            if line.strip():  # Skip empty lines
                line_indent = len(line) - len(line.lstrip())
                # If we find a line at same or lower indentation level, that's likely the end
                if line_indent <= base_indent:
                    end_line = i  # 0-based, so this is the line before
                    break
        
        return end_line + 1  # Convert back to 1-based
    
    def _extract_source_text(self, start_line: int, end_line: int) -> Optional[str]:
        """Extract source text between line numbers (1-based)."""
        if not self.source_lines:
            return None
        
        # Convert to 0-based indexing
        start_idx = max(0, start_line - 1)
        end_idx = min(len(self.source_lines), end_line - 1)
        
        if start_idx >= end_idx:
            return None
        
        return "\n".join(self.source_lines[start_idx:end_idx])


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
    print(f"DEBUG: Registering PythonAdapter from {__file__}")
    from valknut.core.registry import register_language_adapter
    register_language_adapter(PythonAdapter)

# Add class identification
print(f"DEBUG: PythonAdapter class defined at {__file__}:{PythonAdapter.__name__}")