"""
Integration bridge for sibylline-echo clone detection.
"""

import logging
from pathlib import Path
from typing import Dict, List, Optional

from valknut.core.featureset import BaseFeatureExtractor
from valknut.lang.common_ast import Entity, ParseIndex

logger = logging.getLogger(__name__)


class EchoCloneGroup:
    """Wrapper for echo clone group."""
    
    def __init__(self, similarity: float, members: List[Dict[str, any]]) -> None:
        self.similarity = similarity
        self.members = members  # List of {path, start_line, end_line, ...}
    
    def get_member_paths(self) -> List[str]:
        """Get all file paths in this clone group."""
        return [member.get("path", "") for member in self.members]
    
    def get_total_lines(self) -> int:
        """Get total lines across all clone instances."""
        total = 0
        for member in self.members:
            start = member.get("start_line", 0)
            end = member.get("end_line", 0) 
            total += max(0, end - start + 1)
        return total
    
    def contains_entity(self, entity: Entity) -> bool:
        """Check if this clone group contains the given entity."""
        entity_path = str(entity.location.file_path)
        entity_start = entity.location.start_line
        entity_end = entity.location.end_line
        
        for member in self.members:
            member_path = member.get("path", "")
            member_start = member.get("start_line", 0)
            member_end = member.get("end_line", 0)
            
            if (member_path == entity_path and
                member_start <= entity_end and
                member_end >= entity_start):
                return True
        
        return False
    
    def get_overlap_with_entity(self, entity: Entity) -> int:
        """Get number of overlapping lines with entity."""
        entity_path = str(entity.location.file_path)
        entity_start = entity.location.start_line
        entity_end = entity.location.end_line
        
        overlap_lines = 0
        
        for member in self.members:
            member_path = member.get("path", "")
            member_start = member.get("start_line", 0)
            member_end = member.get("end_line", 0)
            
            if member_path == entity_path:
                # Calculate overlap
                overlap_start = max(entity_start, member_start)
                overlap_end = min(entity_end, member_end)
                
                if overlap_start <= overlap_end:
                    overlap_lines += overlap_end - overlap_start + 1
        
        return overlap_lines


class EchoExtractor(BaseFeatureExtractor):
    """Feature extractor using echo for clone detection."""
    
    def __init__(self, min_similarity: float = 0.85, min_tokens: int = 30) -> None:
        self.min_similarity = min_similarity
        self.min_tokens = min_tokens
        self._clone_groups: Optional[List[EchoCloneGroup]] = None
        self._indexed_files: Optional[List[str]] = None
        super().__init__()
    
    @property
    def name(self) -> str:
        return "echo"
    
    def _initialize_features(self) -> None:
        """Initialize clone-related features."""
        self._add_feature(
            "clone_mass",
            "Ratio of duplicated lines to total lines",
            min_value=0.0,
            max_value=1.0,
            default_value=0.0,
        )
        self._add_feature(
            "clone_groups_count",
            "Number of clone groups this entity participates in",
            min_value=0.0,
            max_value=50.0,
            default_value=0.0,
        )
        self._add_feature(
            "max_clone_similarity",
            "Maximum similarity with any clone",
            min_value=0.0,
            max_value=1.0,
            default_value=0.0,
        )
        self._add_feature(
            "clone_locations_count",
            "Total number of clone locations",
            min_value=0.0,
            max_value=100.0,
            default_value=0.0,
        )
    
    def supports_entity(self, entity: Entity) -> bool:
        """Support all entity types that have source code."""
        return entity.raw_text is not None or entity.kind.name == "FILE"
    
    def extract(self, entity: Entity, index: ParseIndex) -> Dict[str, float]:
        """Extract clone features for an entity."""
        if self._clone_groups is None:
            self._initialize_clone_detection(index)
        
        if not self._clone_groups:
            return {f.name: f.default_value for f in self.features}
        
        features = {}
        
        features["clone_mass"] = self._safe_extract(
            entity, index, "clone_mass",
            lambda: self._calculate_clone_mass(entity)
        )
        
        features["clone_groups_count"] = self._safe_extract(
            entity, index, "clone_groups_count",
            lambda: self._count_clone_groups(entity)
        )
        
        features["max_clone_similarity"] = self._safe_extract(
            entity, index, "max_clone_similarity",
            lambda: self._get_max_similarity(entity)
        )
        
        features["clone_locations_count"] = self._safe_extract(
            entity, index, "clone_locations_count", 
            lambda: self._count_clone_locations(entity)
        )
        
        return features
    
    def _initialize_clone_detection(self, index: ParseIndex) -> None:
        """Initialize echo clone detection."""
        try:
            # Import echo here to handle missing dependency gracefully
            # import sibylline_echo as echo  # Not yet available
            raise ImportError("sibylline_echo not available")
            
            # Get all file paths from index
            file_paths = [str(path) for path in index.files.keys()]
            
            if not file_paths:
                logger.warning("No files found for echo clone detection")
                self._clone_groups = []
                return
            
            # Index files with echo
            echo_index = echo.index(file_paths)
            self._indexed_files = file_paths
            
            # Get clone groups
            raw_clone_groups = echo_index.clone_groups(
                min_similarity=self.min_similarity,
                min_tokens=self.min_tokens
            )
            
            # Wrap in our format
            self._clone_groups = []
            for group in raw_clone_groups:
                # Convert group to our format
                similarity = getattr(group, 'similarity', self.min_similarity)
                members = []
                
                if hasattr(group, 'members'):
                    for member in group.members:
                        member_dict = {
                            'path': getattr(member, 'path', ''),
                            'start_line': getattr(member, 'start_line', 0),
                            'end_line': getattr(member, 'end_line', 0),
                        }
                        members.append(member_dict)
                
                if members:
                    self._clone_groups.append(EchoCloneGroup(similarity, members))
            
            logger.info(f"Found {len(self._clone_groups)} clone groups with echo")
            
        except ImportError:
            logger.warning("sibylline-echo not available, clone detection disabled")
            self._clone_groups = []
        except Exception as e:
            logger.error(f"Echo clone detection failed: {e}")
            self._clone_groups = []
    
    def _calculate_clone_mass(self, entity: Entity) -> float:
        """Calculate clone mass ratio for entity."""
        if not self._clone_groups or entity.loc <= 0:
            return 0.0
        
        total_cloned_lines = 0
        
        for group in self._clone_groups:
            if group.contains_entity(entity):
                overlap = group.get_overlap_with_entity(entity)
                total_cloned_lines += overlap
        
        # Avoid double counting overlapping clones
        clone_mass = min(1.0, total_cloned_lines / entity.loc)
        return clone_mass
    
    def _count_clone_groups(self, entity: Entity) -> float:
        """Count number of clone groups entity participates in."""
        if not self._clone_groups:
            return 0.0
        
        count = 0
        for group in self._clone_groups:
            if group.contains_entity(entity):
                count += 1
        
        return float(count)
    
    def _get_max_similarity(self, entity: Entity) -> float:
        """Get maximum similarity with any clone."""
        if not self._clone_groups:
            return 0.0
        
        max_similarity = 0.0
        
        for group in self._clone_groups:
            if group.contains_entity(entity):
                max_similarity = max(max_similarity, group.similarity)
        
        return max_similarity
    
    def _count_clone_locations(self, entity: Entity) -> float:
        """Count total number of clone locations."""
        if not self._clone_groups:
            return 0.0
        
        total_locations = 0
        
        for group in self._clone_groups:
            if group.contains_entity(entity):
                # Count other locations (exclude self)
                total_locations += len(group.members) - 1
        
        return float(total_locations)
    
    def get_clone_briefs(self, entity: Entity) -> List[Dict[str, any]]:
        """Get clone information for brief generation."""
        if not self._clone_groups:
            return []
        
        briefs = []
        
        for group in self._clone_groups:
            if group.contains_entity(entity):
                other_locations = []
                entity_path = str(entity.location.file_path)
                
                for member in group.members:
                    member_path = member.get("path", "")
                    if member_path != entity_path:
                        other_locations.append({
                            "path": member_path,
                            "lines": f"{member.get('start_line', 0)}-{member.get('end_line', 0)}",
                            "similarity": group.similarity,
                        })
                
                if other_locations:
                    briefs.append({
                        "similarity": group.similarity,
                        "locations": other_locations,
                    })
        
        # Sort by similarity descending, limit to top 3
        briefs.sort(key=lambda x: x["similarity"], reverse=True)
        return briefs[:3]


def create_echo_extractor(min_similarity: float = 0.85, min_tokens: int = 30) -> EchoExtractor:
    """Create echo extractor with configuration."""
    return EchoExtractor(min_similarity=min_similarity, min_tokens=min_tokens)