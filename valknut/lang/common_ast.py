"""
Common AST abstractions and shared types for language adapters.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any, Dict, Generator, List, Optional, Protocol, Set, Union

import networkx as nx


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