"""
Common AST abstractions and shared types for language adapters.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any, Dict, Generator, List, Optional, Protocol, Set, Union, Tuple
import logging
import re
from datetime import datetime

try:
    from tree_sitter import Language, Node, Parser
except ImportError:
    Language = None
    Node = None
    Parser = None

import networkx as nx

logger = logging.getLogger(__name__)


class EntityKind(Enum):
    """Kinds of code entities that can be analyzed."""
    
    FILE = "file"
    MODULE = "module"
    CLASS = "class"
    METHOD = "method"
    FUNCTION = "function"
    PROPERTY = "property"
    VARIABLE = "variable"
    INTERFACE = "interface"
    ENUM = "enum"
    STRUCT = "struct"
    TRAIT = "trait"


@dataclass
class SourceLocation:
    """Source code location information."""
    
    file_path: Path
    start_line: int
    end_line: int
    start_column: int = 0
    end_column: int = 0
    
    @property
    def line_count(self) -> int:
        """Number of lines spanned."""
        return self.end_line - self.start_line + 1
    
    def contains(self, other: "SourceLocation") -> bool:
        """Check if this location contains another location."""
        if self.file_path != other.file_path:
            return False
        
        return (
            self.start_line <= other.start_line and
            self.end_line >= other.end_line and
            (self.start_line < other.start_line or self.start_column <= other.start_column) and
            (self.end_line > other.end_line or self.end_column >= other.end_column)
        )


@dataclass
class Entity:
    """Represents a code entity (file, class, function, etc.)."""
    
    id: str  # Unique identifier like "python://pkg/module.py::Class.method"
    name: str
    kind: EntityKind
    location: SourceLocation
    language: str
    parent_id: Optional[str] = None
    children: List[str] = field(default_factory=list)
    
    # Source code information
    raw_text: Optional[str] = None
    signature: Optional[str] = None
    docstring: Optional[str] = None
    
    # Structural information
    parameters: List[str] = field(default_factory=list)
    return_type: Optional[str] = None
    fields: List[str] = field(default_factory=list)
    imports: List[str] = field(default_factory=list)
    
    # Metrics (will be populated by detectors)
    metrics: Dict[str, Any] = field(default_factory=dict)
    
    @property
    def loc(self) -> int:
        """Lines of code."""
        return self.location.line_count
    
    @property
    def qualified_name(self) -> str:
        """Get the qualified name of this entity."""
        if "::" in self.id:
            return self.id.split("::", 1)[1]
        return self.name


@dataclass
class ParseIndex:
    """Index of parsed files and entities."""
    
    entities: Dict[str, Entity]
    files: Dict[Path, str]  # file_path -> entity_id mapping
    import_graph: Optional[nx.DiGraph] = None
    call_graph: Optional[nx.DiGraph] = None
    
    # Cache for lookups
    _by_kind: Dict[EntityKind, List[str]] = field(default_factory=dict)
    _by_file: Dict[Path, List[str]] = field(default_factory=dict)
    
    def __post_init__(self) -> None:
        """Build lookup caches."""
        self._rebuild_caches()
    
    def _rebuild_caches(self) -> None:
        """Rebuild lookup caches."""
        self._by_kind.clear()
        self._by_file.clear()
        
        for entity_id, entity in self.entities.items():
            # By kind
            if entity.kind not in self._by_kind:
                self._by_kind[entity.kind] = []
            self._by_kind[entity.kind].append(entity_id)
            
            # By file
            if entity.location.file_path not in self._by_file:
                self._by_file[entity.location.file_path] = []
            self._by_file[entity.location.file_path].append(entity_id)
    
    def get_by_kind(self, kind: EntityKind) -> List[Entity]:
        """Get all entities of a specific kind."""
        entity_ids = self._by_kind.get(kind, [])
        return [self.entities[eid] for eid in entity_ids]
    
    def get_by_file(self, file_path: Path) -> List[Entity]:
        """Get all entities in a specific file."""
        entity_ids = self._by_file.get(file_path, [])
        return [self.entities[eid] for eid in entity_ids]
    
    def get_children(self, entity_id: str) -> List[Entity]:
        """Get direct children of an entity."""
        if entity_id not in self.entities:
            return []
        
        return [
            self.entities[child_id] 
            for child_id in self.entities[entity_id].children
            if child_id in self.entities
        ]
    
    def get_parent(self, entity_id: str) -> Optional[Entity]:
        """Get parent of an entity."""
        if entity_id not in self.entities:
            return None
        
        parent_id = self.entities[entity_id].parent_id
        if parent_id and parent_id in self.entities:
            return self.entities[parent_id]
        return None


class LanguageAdapter(Protocol):
    """Protocol for language-specific adapters."""
    
    @property
    def language(self) -> str:
        """Language name."""
        ...
    
    @property
    def file_extensions(self) -> Set[str]:
        """Supported file extensions."""
        ...
    
    def discover(self, roots: List[str], include_patterns: List[str], exclude_patterns: List[str]) -> List[Path]:
        """
        Discover files to analyze.
        
        Args:
            roots: Root directories to search
            include_patterns: Include glob patterns
            exclude_patterns: Exclude glob patterns
            
        Returns:
            List of discovered files
        """
        ...
    
    def parse_index(self, files: List[Path]) -> ParseIndex:
        """
        Parse files and build index.
        
        Args:
            files: Files to parse
            
        Returns:
            Parse index with entities and graphs
        """
        ...
    
    def entities(self, index: ParseIndex) -> Generator[Entity, None, None]:
        """
        Generate entities for analysis.
        
        Args:
            index: Parse index
            
        Yields:
            Entities to analyze
        """
        ...
    
    def call_graph(self, index: ParseIndex) -> Optional[nx.DiGraph]:
        """
        Build call graph if supported.
        
        Args:
            index: Parse index
            
        Returns:
            Call graph or None if not supported
        """
        ...
    
    def import_graph(self, index: ParseIndex) -> nx.DiGraph:
        """
        Build import/dependency graph.
        
        Args:
            index: Parse index
            
        Returns:
            Import graph
        """
        ...
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """
        Extract type-related features.
        
        Args:
            entity: Entity to analyze
            
        Returns:
            Dictionary of type features
        """
        ...
    
    def exception_features(self, entity: Entity) -> Dict[str, float]:
        """
        Extract exception-related features.
        
        Args:
            entity: Entity to analyze
            
        Returns:
            Dictionary of exception features
        """
        ...
    
    def cohesion_features(self, entity: Entity) -> Dict[str, float]:
        """
        Extract cohesion-related features.
        
        Args:
            entity: Entity to analyze
            
        Returns:
            Dictionary of cohesion features
        """
        ...


class BaseLanguageAdapter(ABC):
    """Base class for language adapters."""
    
    def __init__(self) -> None:
        self._file_cache: Dict[Path, str] = {}
    
    @property
    @abstractmethod
    def language(self) -> str:
        """Language name."""
        ...
    
    @property
    @abstractmethod
    def file_extensions(self) -> Set[str]:
        """Supported file extensions."""
        ...
    
    def _read_file(self, file_path: Path) -> str:
        """Read file with caching."""
        if file_path not in self._file_cache:
            try:
                with file_path.open("r", encoding="utf-8") as f:
                    self._file_cache[file_path] = f.read()
            except Exception as e:
                raise ValueError(f"Failed to read {file_path}: {e}") from e
        return self._file_cache[file_path]
    
    def _make_entity_id(self, file_path: Path, qualified_name: Optional[str] = None) -> str:
        """Create entity ID."""
        base = f"{self.language}://{file_path}"
        if qualified_name:
            base += f"::{qualified_name}"
        return base
    
    def _extract_signature(self, entity: Entity) -> Optional[str]:
        """Extract signature from entity (to be implemented by subclasses)."""
        return None


@dataclass
class ParsedImport:
    """Represents a parsed import statement."""
    module: str
    imported_names: List[str] = field(default_factory=list)
    alias: Optional[str] = None
    is_relative: bool = False
    source_location: Optional[SourceLocation] = None


@dataclass
class AdapterDiagnostic:
    """Represents a diagnostic message from a language adapter."""
    level: str  # "error", "warning", "info"
    message: str
    file_path: Optional[Path] = None
    line: Optional[int] = None
    column: Optional[int] = None
    timestamp: datetime = field(default_factory=datetime.now)


@dataclass
class AdapterStatus:
    """Represents the status and capabilities of a language adapter."""
    language: str
    available: bool
    parser_module: str
    parser_function: str
    tree_sitter_available: bool
    file_extensions: Set[str]
    supported_features: Set[str] = field(default_factory=set)
    diagnostics: List[AdapterDiagnostic] = field(default_factory=list)
    initialization_error: Optional[str] = None
    
    def add_diagnostic(self, level: str, message: str, file_path: Optional[Path] = None, 
                      line: Optional[int] = None, column: Optional[int] = None) -> None:
        """Add a diagnostic message."""
        self.diagnostics.append(AdapterDiagnostic(
            level=level, 
            message=message, 
            file_path=file_path, 
            line=line, 
            column=column
        ))
    
    def has_errors(self) -> bool:
        """Check if there are any error-level diagnostics."""
        return any(d.level == "error" for d in self.diagnostics)
    
    def has_warnings(self) -> bool:
        """Check if there are any warning-level diagnostics."""
        return any(d.level == "warning" for d in self.diagnostics)


class LanguageSupportRegistry:
    """Registry for tracking language support status across all adapters."""
    
    def __init__(self) -> None:
        self._adapters: Dict[str, AdapterStatus] = {}
    
    def register_adapter_status(self, status: AdapterStatus) -> None:
        """Register an adapter's status."""
        self._adapters[status.language] = status
    
    def get_status(self, language: str) -> Optional[AdapterStatus]:
        """Get status for a specific language."""
        return self._adapters.get(language)
    
    def get_all_statuses(self) -> Dict[str, AdapterStatus]:
        """Get all registered adapter statuses."""
        return self._adapters.copy()
    
    def get_available_languages(self) -> Set[str]:
        """Get set of languages with available adapters."""
        return {lang for lang, status in self._adapters.items() if status.available}
    
    def get_unavailable_languages(self) -> Dict[str, str]:
        """Get languages with unavailable adapters and their reasons."""
        return {
            lang: status.initialization_error or "Unknown error"
            for lang, status in self._adapters.items() 
            if not status.available
        }
    
    def generate_support_report(self) -> str:
        """Generate a human-readable support report."""
        lines = ["Language Support Status:", "=" * 25, ""]
        
        available = []
        unavailable = []
        
        for lang, status in sorted(self._adapters.items()):
            if status.available:
                features = ", ".join(sorted(status.supported_features)) if status.supported_features else "Basic parsing"
                available.append(f"  ✓ {lang.capitalize()}: {features}")
            else:
                reason = status.initialization_error or "Parser module not available"
                unavailable.append(f"  ✗ {lang.capitalize()}: {reason}")
        
        if available:
            lines.extend(["Available Languages:"] + available + [""])
        
        if unavailable:
            lines.extend(["Unavailable Languages:"] + unavailable + [""])
        
        # Add diagnostics summary
        total_errors = sum(len([d for d in status.diagnostics if d.level == "error"]) 
                          for status in self._adapters.values())
        total_warnings = sum(len([d for d in status.diagnostics if d.level == "warning"]) 
                            for status in self._adapters.values())
        
        lines.extend([
            f"Diagnostics Summary:",
            f"  Errors: {total_errors}",
            f"  Warnings: {total_warnings}"
        ])
        
        return "\n".join(lines)


# Global registry instance
language_support_registry = LanguageSupportRegistry()


class TreeSitterBaseAdapter(BaseLanguageAdapter):
    """Base class for tree-sitter based language adapters."""
    
    def __init__(self) -> None:
        super().__init__()
        self._language: Optional["Language"] = None
        self._parser: Optional["Parser"] = None
        self._node_type_map: Dict[str, EntityKind] = {}
        self._import_patterns: List[str] = []
        self._status = self._create_initial_status()
        self._available = self._initialize_parser()
        self._register_status()
    
    @property
    def available(self) -> bool:
        """Check if the parser is available and ready to use."""
        return self._available
    
    @property 
    def status(self) -> AdapterStatus:
        """Get the adapter status."""
        return self._status
    
    @property
    @abstractmethod
    def parser_module(self) -> str:
        """Import name for the tree-sitter parser module (e.g., 'tree_sitter_python')."""
        ...
    
    @property
    @abstractmethod
    def parser_language_function(self) -> str:
        """Function name to get the language from parser module (e.g., 'language')."""
        ...
    
    @property
    def node_type_mapping(self) -> Dict[str, EntityKind]:
        """Mapping from tree-sitter node types to EntityKind."""
        return self._node_type_map
    
    @property
    def import_patterns(self) -> List[str]:
        """Regex patterns for extracting import statements."""
        return self._import_patterns
    
    def _create_initial_status(self) -> AdapterStatus:
        """Create initial adapter status."""
        return AdapterStatus(
            language=self.language,
            available=False,  # Will be updated during initialization
            parser_module=self.parser_module,
            parser_function=self.parser_language_function,
            tree_sitter_available=Language is not None and Parser is not None,
            file_extensions=self.file_extensions,
        )
    
    def _register_status(self) -> None:
        """Register this adapter's status with the global registry."""
        language_support_registry.register_adapter_status(self._status)
    
    def _initialize_parser(self) -> bool:
        """Initialize the tree-sitter parser."""
        if Language is None or Parser is None:
            error_msg = "tree-sitter not available - please install tree-sitter package"
            logger.warning(error_msg)
            self._status.initialization_error = error_msg
            self._status.available = False
            return False
        
        try:
            # Dynamically import the parser module
            parser_module = __import__(self.parser_module)
            language_func = getattr(parser_module, self.parser_language_function)
            
            self._language = Language(language_func())
            self._parser = Parser(self._language)
            self._setup_mappings()
            
            # Update status on success
            self._status.available = True
            self._status.supported_features.update({
                "parsing", "entity_extraction", "import_resolution", 
                "type_features", "exception_features", "cohesion_features"
            })
            self._status.add_diagnostic("info", f"Successfully initialized {self.language} parser")
            
            return True
            
        except ImportError as e:
            error_msg = f"{self.parser_module} not available: {e}"
            logger.warning(error_msg)
            self._status.initialization_error = error_msg
            self._status.available = False
            self._status.add_diagnostic("error", error_msg)
            return False
        except Exception as e:
            error_msg = f"Failed to initialize {self.language} parser: {e}"
            logger.warning(error_msg)
            self._status.initialization_error = error_msg
            self._status.available = False
            self._status.add_diagnostic("error", error_msg)
            return False
    
    @abstractmethod
    def _setup_mappings(self) -> None:
        """Setup node type mappings and import patterns (to be implemented by subclasses)."""
        ...
    
    def parse_index(self, files: List[Path]) -> ParseIndex:
        """Parse files and build index using tree-sitter."""
        if not self._available or self._parser is None:
            error_msg = f"{self.language} parser not available"
            logger.warning(error_msg)
            self._status.add_diagnostic("error", error_msg)
            return ParseIndex({}, {}, nx.DiGraph(), nx.DiGraph())
        
        entities: Dict[str, Entity] = {}
        file_mapping: Dict[Path, str] = {}
        
        for file_path in files:
            try:
                file_entities = self._parse_file(file_path)
                
                # Add file entity
                file_entity_id = self._make_entity_id(file_path)
                file_content = self._read_file(file_path)
                file_entity = Entity(
                    id=file_entity_id,
                    name=file_path.name,
                    kind=EntityKind.FILE,
                    location=SourceLocation(file_path, 1, len(file_content.splitlines()) if file_content else 1, 0, 0),
                    language=self.language,
                    raw_text=file_content,
                )
                
                entities[file_entity_id] = file_entity
                file_mapping[file_path] = file_entity_id
                
                # Add parsed entities and set relationships
                for entity in file_entities:
                    entities[entity.id] = entity
                    
                    if entity.parent_id:
                        # Entity has a parent in the same file
                        if entity.parent_id in entities:
                            entities[entity.parent_id].children.append(entity.id)
                    else:
                        # Top-level entity, parent is file
                        entity.parent_id = file_entity_id
                        file_entity.children.append(entity.id)
                
            except Exception as e:
                error_msg = f"Failed to parse {file_path}: {e}"
                logger.warning(error_msg)
                self._status.add_diagnostic("error", error_msg, file_path=file_path)
        
        # Build graphs
        import_graph = self._build_import_graph(entities)
        call_graph = self._build_call_graph(entities)
        
        # Log graph statistics for debugging
        logger.info(
            f"Graph analysis for {len(entities)} entities: "
            f"import_graph({import_graph.number_of_nodes()} nodes, {import_graph.number_of_edges()} edges), "
            f"call_graph({call_graph.number_of_nodes() if call_graph else 0} nodes, "
            f"{call_graph.number_of_edges() if call_graph else 0} edges)"
        )
        
        return ParseIndex(entities, file_mapping, import_graph, call_graph)
    
    def _parse_file(self, file_path: Path) -> List[Entity]:
        """Parse a single file using tree-sitter."""
        if not self._available or self._parser is None:
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
            error_msg = f"Failed to parse {file_path}: {e}"
            logger.warning(error_msg)
            self._status.add_diagnostic("error", error_msg, file_path=file_path)
            return []
    
    def _extract_entities(self, node: "Node", file_path: Path, content: str, entities: List[Entity], parent_id: Optional[str] = None) -> None:
        """Extract entities from tree-sitter node."""
        if node.type in self._node_type_map:
            entity_kind = self._node_type_map[node.type]
            entity_name = self._extract_name(node)
            
            if entity_name:
                # Build qualified name if we have a parent
                if parent_id:
                    qualified_name = f"{parent_id.split('::')[-1]}.{entity_name}"
                else:
                    qualified_name = entity_name
                    
                entity_id = self._make_entity_id(file_path, qualified_name)
                
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
                
                # Extract additional information
                parameters = self._extract_parameters(node)
                signature = self._extract_signature_from_node(node, content)
                docstring = self._extract_docstring(node, content)
                
                entity = Entity(
                    id=entity_id,
                    name=entity_name,
                    kind=entity_kind,
                    location=location,
                    language=self.language,
                    parent_id=parent_id,
                    parameters=parameters,
                    raw_text=raw_text,
                    signature=signature,
                    docstring=docstring,
                )
                
                entities.append(entity)
                parent_id = entity_id  # Children will have this as parent
        
        # Recursively process child nodes
        for child in node.children:
            self._extract_entities(child, file_path, content, entities, parent_id)
    
    def _extract_name(self, node: "Node") -> Optional[str]:
        """Extract name from a node. Can be overridden by subclasses for language-specific logic."""
        # Common identifier patterns
        identifier_types = ["identifier", "property_identifier", "type_identifier", "field_identifier"]
        
        for child in node.children:
            if child.type in identifier_types:
                return child.text.decode("utf8")
        
        return None
    
    def _extract_parameters(self, node: "Node") -> List[str]:
        """Extract parameter names from function/method node. Can be overridden by subclasses."""
        parameters = []
        
        # Look for parameter-related nodes
        parameter_containers = ["formal_parameters", "parameters", "parameter_list"]
        parameter_types = ["parameter", "required_parameter", "optional_parameter", "identifier"]
        
        for child in node.children:
            if child.type in parameter_containers:
                self._extract_parameters_from_container(child, parameters, parameter_types)
        
        return parameters
    
    def _extract_parameters_from_container(self, container: "Node", parameters: List[str], parameter_types: List[str]) -> None:
        """Extract parameters from a parameter container node."""
        for param_node in container.children:
            if param_node.type in parameter_types:
                # For complex parameter nodes, look for identifier
                param_name = None
                if param_node.type == "identifier":
                    param_name = param_node.text.decode("utf8")
                else:
                    # Look deeper for identifier
                    for param_child in param_node.children:
                        if param_child.type == "identifier":
                            param_name = param_child.text.decode("utf8")
                            break
                
                if param_name:
                    parameters.append(param_name)
    
    def _extract_signature_from_node(self, node: "Node", content: str) -> Optional[str]:
        """Extract signature from node (can be overridden by subclasses)."""
        # Default implementation - just get the first line
        start_line = node.start_point.row
        end_line = node.end_point.row
        
        if start_line == end_line:
            # Single line
            return content[node.start_byte:node.end_byte]
        else:
            # Multi-line, just get first line or first few lines for signature
            lines = content.splitlines()
            if start_line < len(lines):
                # Try to find the end of the signature (e.g., closing parenthesis)
                signature_lines = []
                for i in range(start_line, min(end_line + 1, len(lines))):
                    signature_lines.append(lines[i])
                    if ')' in lines[i] and not lines[i].strip().endswith(','):
                        break
                return '\n'.join(signature_lines)
        
        return None
    
    def _extract_docstring(self, node: "Node", content: str) -> Optional[str]:
        """Extract docstring from node (can be overridden by subclasses)."""
        # This is a basic implementation - subclasses can provide language-specific logic
        return None
    
    def _build_import_graph(self, entities: Dict[str, Entity]) -> nx.DiGraph:
        """Build import dependency graph using tree-sitter parsing."""
        graph = nx.DiGraph()
        
        # Add all file entities as nodes
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                graph.add_node(entity_id, path=str(entity.location.file_path))
        
        # Parse imports from each file
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE and entity.raw_text:
                imports = self._extract_imports_from_content(entity.raw_text, entity.location.file_path)
                
                for parsed_import in imports:
                    # Try to resolve import to actual file
                    imported_entity_id = self._resolve_import(parsed_import, entity.location.file_path, entities)
                    if imported_entity_id and imported_entity_id != entity_id:
                        graph.add_edge(entity_id, imported_entity_id, import_data=parsed_import)
        
        return graph
    
    def _extract_imports_from_content(self, content: str, file_path: Path) -> List[ParsedImport]:
        """Extract imports using both regex patterns and tree-sitter if available."""
        imports = []
        
        # Use regex patterns for basic extraction
        for pattern in self._import_patterns:
            matches = re.finditer(pattern, content, re.MULTILINE)
            for match in matches:
                imports.append(self._create_parsed_import_from_regex(match, file_path, content))
        
        return imports
    
    def _create_parsed_import_from_regex(self, match: re.Match, file_path: Path, content: str) -> ParsedImport:
        """Create ParsedImport from regex match with enhanced parsing."""
        if not match.groups():
            return ParsedImport(module=match.group(0))
        
        module_name = match.group(1)
        
        # Detect if it's a relative import
        is_relative = module_name.startswith(".") 
        
        # Extract imported names if it's a 'from ... import ...' statement  
        imported_names = []
        full_match = match.group(0)
        if "import" in full_match and "from" in full_match:
            # Try to extract what's being imported
            import_part = full_match.split("import", 1)[1] if "import" in full_match else ""
            if import_part:
                # Basic parsing of imported items
                import_items = [item.strip() for item in import_part.split(",")]
                imported_names = [item.split(" as ")[0].strip() for item in import_items if item.strip()]
        
        return ParsedImport(
            module=module_name,
            imported_names=imported_names,
            is_relative=is_relative
        )
    
    def _resolve_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve import to entity ID with enhanced logic."""
        # Try relative resolution first
        if parsed_import.is_relative or parsed_import.module.startswith("."):
            return self._resolve_relative_import(parsed_import, current_file, entities)
        
        # Try absolute resolution
        return self._resolve_absolute_import(parsed_import, entities)
    
    def _resolve_relative_import(self, parsed_import: ParsedImport, current_file: Path, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve relative import (can be overridden by subclasses)."""
        # Basic implementation - can be enhanced per language
        module_path = parsed_import.module.lstrip(".")
        potential_paths = []
        
        # Try different extensions
        for ext in self.file_extensions:
            potential_paths.append(current_file.parent / f"{module_path}{ext}")
            potential_paths.append(current_file.parent / module_path / f"__init__{ext}")
        
        for potential_path in potential_paths:
            for entity_id, entity in entities.items():
                if entity.kind == EntityKind.FILE and entity.location.file_path == potential_path.resolve():
                    return entity_id
        
        return None
    
    def _resolve_absolute_import(self, parsed_import: ParsedImport, entities: Dict[str, Entity]) -> Optional[str]:
        """Resolve absolute import with improved matching logic."""
        module_parts = parsed_import.module.split(".")
        
        # Try different resolution strategies
        for entity_id, entity in entities.items():
            if entity.kind == EntityKind.FILE:
                file_path = entity.location.file_path
                
                # Strategy 1: Direct module name match
                file_stem = file_path.stem
                if file_stem == module_parts[-1]:
                    return entity_id
                
                # Strategy 2: Package path match (e.g., "valknut.core.config" -> valknut/core/config.py)
                module_path_str = "/".join(module_parts)
                if module_path_str in str(file_path):
                    return entity_id
                
                # Strategy 3: Partial path match
                if len(module_parts) > 1:
                    partial_path = "/".join(module_parts[-2:])
                    if partial_path in str(file_path):
                        return entity_id
                
                # Strategy 4: Handle __init__ files for package imports
                if file_stem == "__init__" and len(module_parts) > 0:
                    package_name = module_parts[-1]
                    if package_name == file_path.parent.name:
                        return entity_id
        
        return None
    
    def _build_call_graph(self, entities: Dict[str, Entity]) -> nx.DiGraph:
        """Build call dependency graph with basic function call detection."""
        graph = nx.DiGraph()
        
        # Add function/method entities as nodes
        functions = {}
        for entity_id, entity in entities.items():
            if entity.kind in {EntityKind.FUNCTION, EntityKind.METHOD}:
                graph.add_node(entity_id, **entity.__dict__)
                functions[entity.name] = entity_id
        
        # Detect function calls using regex patterns
        # This is a basic implementation - subclasses can provide more sophisticated analysis
        call_patterns = [
            r'\b(\w+)\s*\(',  # Simple function call: function_name(
            r'\.(\w+)\s*\(',  # Method call: obj.method(
        ]
        
        for entity_id, entity in entities.items():
            if entity.kind in {EntityKind.FUNCTION, EntityKind.METHOD, EntityKind.FILE}:
                if not entity.raw_text:
                    continue
                    
                # Look for function calls in the entity's text
                for pattern in call_patterns:
                    matches = re.finditer(pattern, entity.raw_text, re.MULTILINE)
                    for match in matches:
                        called_name = match.group(1)
                        
                        # Try to find the called function in our entities
                        if called_name in functions:
                            called_entity_id = functions[called_name]
                            
                            # Don't create self-loops unless it's actually recursive
                            if called_entity_id != entity_id:
                                graph.add_edge(entity_id, called_entity_id, call_type="function_call")
                            elif "recursive" in entity.raw_text.lower() or called_name in entity.raw_text[entity.raw_text.find(called_name) + len(called_name):]:
                                # Basic recursion detection
                                graph.add_edge(entity_id, called_entity_id, call_type="recursive_call")
        
        return graph
    
    def entities(self, index: ParseIndex) -> Generator[Entity, None, None]:
        """Generate entities for analysis."""
        for entity in index.entities.values():
            yield entity
    
    def call_graph(self, index: ParseIndex) -> Optional[nx.DiGraph]:
        """Build call graph if supported."""
        return index.call_graph
    
    def import_graph(self, index: ParseIndex) -> nx.DiGraph:
        """Build import/dependency graph."""
        return index.import_graph or nx.DiGraph()
    
    def type_features(self, entity: Entity) -> Dict[str, float]:
        """Extract type-related features (can be overridden by subclasses)."""
        return {}
    
    def exception_features(self, entity: Entity) -> Dict[str, float]:
        """Extract exception-related features (can be overridden by subclasses)."""
        return {}
    
    def cohesion_features(self, entity: Entity) -> Dict[str, float]:
        """Extract cohesion-related features (can be overridden by subclasses)."""
        return {"lcom_like": 0.0}