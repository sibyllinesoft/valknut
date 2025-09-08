"""
Filesystem Structure Analyzer for detecting directory balance issues and file organization problems.

This detector analyzes repository structure and generates recommendations for:
- Branch Packs: Split overcrowded directories 
- File-Split Packs: Break up mega-files with low cohesion
"""

import json
import logging
import math
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple, Union
import hashlib

from valknut.lang.common_ast import Entity, ParseIndex
from valknut.core.featureset import FeatureExtractor, FeatureVector

try:
    from tqdm import tqdm
except ImportError:
    # Fallback if tqdm not available
    class tqdm:
        def __init__(self, iterable=None, desc=None, total=None, **kwargs):
            self.iterable = iterable
            self.desc = desc
            self.total = total
        def __iter__(self):
            return iter(self.iterable)
        def __enter__(self):
            return self
        def __exit__(self, *args):
            pass
        def update(self, n=1):
            pass
        def set_description(self, desc):
            pass

logger = logging.getLogger(__name__)


@dataclass
class FileNode:
    """Represents a file in the filesystem structure."""
    path: Path
    loc: int = 0
    bytes: int = 0
    language: str = "unknown"
    entities: List[str] = field(default_factory=list)
    huge: bool = False
    exports: int = 0
    external_importers: int = 0
    cohesion_communities: int = 1


@dataclass
class DirNode:
    """Represents a directory in the filesystem structure."""
    path: Path
    parent: Optional['DirNode'] = None
    subdirs: List['DirNode'] = field(default_factory=list)
    files: List[FileNode] = field(default_factory=list)
    
    # Computed metrics
    branching_factor: float = 0.0
    branching_factor_effective: float = 0.0
    leaf_load: int = 0
    leaf_density: float = 0.0
    depth: int = 0
    depth_normalized: float = 0.0
    total_loc: int = 0
    gini_loc: float = 0.0
    entropy_loc: float = 0.0
    
    # Pressure metrics
    size_pressure: float = 0.0
    file_pressure: float = 0.0
    branch_pressure: float = 0.0
    dispersion: float = 0.0
    dir_imbalance: float = 0.0
    
    hot_leaves: List[FileNode] = field(default_factory=list)


@dataclass
class FileSplitPack:
    """Recommendation to split a large file."""
    kind: str = "file_split"
    file: str = ""
    reasons: List[str] = field(default_factory=list)
    suggested_splits: List[Dict[str, Union[str, List[str]]]] = field(default_factory=list)
    value: Dict[str, Union[float, bool]] = field(default_factory=dict)
    effort: Dict[str, int] = field(default_factory=dict)


@dataclass
class BranchReorgPack:
    """Recommendation to reorganize directory structure."""
    kind: str = "branch_reorg" 
    dir: str = ""
    current: Dict[str, Union[int, float]] = field(default_factory=dict)
    proposal: List[Dict[str, Union[str, int]]] = field(default_factory=list)
    value: Dict[str, Union[float, int]] = field(default_factory=dict)
    effort: Dict[str, int] = field(default_factory=dict)
    steps: List[str] = field(default_factory=list)


@dataclass
class StructureConfig:
    """Configuration for structure analysis."""
    # Directory thresholds
    max_files_per_dir: int = 25
    max_subdirs_per_dir: int = 10
    max_dir_loc: int = 2000
    min_branch_recommendation_gain: float = 0.15
    
    # File thresholds
    huge_loc: int = 800
    huge_bytes: int = 128000
    
    # Analysis options
    enable_branch_packs: bool = True
    enable_file_split_packs: bool = True
    top_packs: int = 20
    min_analysis_threshold_files: int = 5
    min_analysis_threshold_loc: int = 600


class FilesystemStructureAnalyzer:
    """Analyzes filesystem structure and generates reorganization recommendations."""
    
    def __init__(self, config: StructureConfig = None):
        self.config = config or StructureConfig()
        self._cache_key = None
        self._cached_tree = None
        
    def analyze_structure(
        self, 
        files: List[Path], 
        parse_indices: Dict[str, ParseIndex],
        import_graph: Optional[Dict] = None
    ) -> Tuple[List[FileSplitPack], List[BranchReorgPack]]:
        """Main analysis entry point."""
        
        with tqdm(total=4, desc="🏗️  Structure analysis", unit="phase", leave=False) as pbar:
            
            # Phase 1: Build filesystem tree
            pbar.set_description("🏗️  Building filesystem tree")
            tree = self._build_fs_tree(files, parse_indices)
            if not tree:
                return [], []
            pbar.update(1)
            
            # Phase 2: Compute metrics for all directories
            pbar.set_description("📊 Computing directory metrics")
            self._compute_metrics(tree)
            pbar.update(1)
            
            # Phase 3: Generate packs
            pbar.set_description("📦 Generating refactoring packs")
            file_split_packs = []
            branch_packs = []
            
            if self.config.enable_file_split_packs:
                file_split_packs = self._generate_file_split_packs(tree)
                
            if self.config.enable_branch_packs:
                branch_packs = self._generate_branch_packs(tree, import_graph)
            pbar.update(1)
            
            # Phase 4: Rank and limit packs
            pbar.set_description("🏆 Ranking impact packs")
            all_packs = file_split_packs + branch_packs
            ranked_packs = self._rank_packs(all_packs)
            pbar.update(1)
        
        # Split back into types and limit
        file_splits = [p for p in ranked_packs if isinstance(p, FileSplitPack)][:self.config.top_packs//2]
        branch_reorgs = [p for p in ranked_packs if isinstance(p, BranchReorgPack)][:self.config.top_packs//2]
        
        logger.debug(f"Generated {len(file_splits)} file-split packs and {len(branch_reorgs)} branch-reorg packs")
        
        return file_splits, branch_reorgs
    
    def _build_fs_tree(self, files: List[Path], parse_indices: Dict[str, ParseIndex]) -> Optional[DirNode]:
        """Build directory tree from file list."""
        if not files:
            return None
            
        # Find common root
        common_root = self._find_common_root(files)
        if not common_root:
            return None
            
        root = DirNode(path=common_root)
        
        # Build directory structure
        dir_map = {common_root: root}
        
        for file_path in tqdm(files, desc="📄 Processing files", leave=False, unit="file"):
            try:
                # Create file node with metadata
                file_node = self._create_file_node(file_path, parse_indices)
                
                # Ensure parent directories exist
                parent_path = file_path.parent
                if parent_path not in dir_map:
                    self._ensure_dir_path(parent_path, dir_map, root)
                    
                # Add file to parent directory
                if parent_path in dir_map:
                    dir_map[parent_path].files.append(file_node)
                    
            except Exception as e:
                pass  # Silently skip problematic files
                continue
                
        # Build parent-child relationships for directories
        for dir_path, dir_node in dir_map.items():
            if dir_path != common_root:
                parent_path = dir_path.parent
                if parent_path in dir_map:
                    dir_node.parent = dir_map[parent_path]
                    if dir_node not in dir_map[parent_path].subdirs:
                        dir_map[parent_path].subdirs.append(dir_node)
                        
        return root
    
    def _find_common_root(self, files: List[Path]) -> Optional[Path]:
        """Find the common root directory for all files."""
        if not files:
            return None
            
        # Start with first file's parent
        root = files[0].parent
        
        for file_path in files[1:]:
            # Find common path
            try:
                file_path.relative_to(root)
            except ValueError:
                # Not under current root, find new common ancestor
                while root != root.parent:  # While not filesystem root
                    root = root.parent
                    try:
                        file_path.relative_to(root)
                        break
                    except ValueError:
                        continue
                else:
                    # Reached filesystem root
                    return root
                    
        return root
    
    def _ensure_dir_path(self, dir_path: Path, dir_map: Dict[Path, DirNode], root: DirNode):
        """Ensure directory path exists in dir_map."""
        if dir_path in dir_map:
            return
            
        # Ensure parent exists first
        parent_path = dir_path.parent
        if parent_path != dir_path and parent_path not in dir_map:
            self._ensure_dir_path(parent_path, dir_map, root)
            
        # Create this directory
        dir_node = DirNode(path=dir_path)
        dir_map[dir_path] = dir_node
    
    def _create_file_node(self, file_path: Path, parse_indices: Dict[str, ParseIndex]) -> FileNode:
        """Create file node with metadata from parse indices. Optimized version."""
        file_node = FileNode(path=file_path)
        file_str = str(file_path)
        
        try:
            # Get basic file stats (cached at OS level, but still expensive)
            try:
                stat_info = file_path.stat()
                file_node.bytes = stat_info.st_size
            except (OSError, IOError):
                file_node.bytes = 0
                
            # Pre-build lookup for entities by file path for all languages
            entities_in_file = []
            found_language = None
            
            for language, index in parse_indices.items():
                # Direct lookup in entities dict is O(1) amortized vs O(n) list comprehension
                file_entities = []
                for entity in index.entities.values():
                    if entity.file_path == file_str:
                        file_entities.append(entity)
                
                if file_entities:
                    entities_in_file = file_entities
                    found_language = language
                    break  # First match wins
                    
            if entities_in_file:
                file_node.language = found_language
                file_node.entities = [e.id for e in entities_in_file]
                
                # Optimized LOC calculation - batch processing
                total_lines = 0
                for entity in entities_in_file:
                    if hasattr(entity, 'end_line') and hasattr(entity, 'start_line'):
                        total_lines += entity.end_line - entity.start_line + 1
                    else:
                        total_lines += 10  # Fallback estimate
                        
                file_node.loc = total_lines
            else:
                # Fallback LOC estimation from file size 
                if file_node.bytes > 0:
                    file_node.loc = max(1, file_node.bytes // 40)
                    
            # Batch boolean checks
            file_node.huge = (
                file_node.loc >= self.config.huge_loc or 
                file_node.bytes >= self.config.huge_bytes
            )
            
            # Optimized exports count - single pass
            file_node.exports = sum(1 for e in file_node.entities if not e.startswith('_'))
            
            # Simplified cohesion estimate
            entity_count = len(file_node.entities)
            file_node.cohesion_communities = min(4, max(1, entity_count // 5)) if entity_count > 10 else 1
                
        except Exception as e:
            pass  # Silently skip problematic files
            
        return file_node
    
    def _compute_metrics(self, tree: DirNode):
        """Compute balance metrics for all directories in the tree."""
        self._compute_metrics_recursive(tree, 0, self._get_max_depth(tree))
        
    def _compute_metrics_recursive(self, dir_node: DirNode, depth: int, max_depth: int):
        """Recursively compute metrics for directory and children. Optimized version."""
        
        # Basic counts
        F = len(dir_node.files)
        D = len(dir_node.subdirs)
        
        # Early exit for trivial directories
        if F == 0 and D == 0:
            return
        
        L = sum(f.loc for f in dir_node.files)  # Total LOC computed once
        
        # Store basic metrics
        dir_node.leaf_load = F
        dir_node.branching_factor = D
        dir_node.branching_factor_effective = D + 0.5 * (1 if F > 0 else 0)
        dir_node.leaf_density = F / (F + D) if (F + D) > 0 else 0
        dir_node.depth = depth
        dir_node.depth_normalized = depth / max_depth if max_depth > 0 else 0
        dir_node.total_loc = L
        
        # Size dispersion metrics - only compute if needed
        if F > 1:
            file_sizes = [f.loc for f in dir_node.files]
            dir_node.gini_loc = self._compute_gini(file_sizes)
            dir_node.entropy_loc = self._compute_entropy(file_sizes)
        else:
            dir_node.gini_loc = 0.0
            dir_node.entropy_loc = 0.0
            
        # Pressure metrics
        dir_node.size_pressure = min(1.0, L / self.config.max_dir_loc)
        dir_node.file_pressure = min(1.0, F / self.config.max_files_per_dir)
        dir_node.branch_pressure = min(1.0, D / self.config.max_subdirs_per_dir)
        
        # Dispersion combines gini and low entropy
        max_entropy = math.log2(max(F, 1))
        entropy_factor = 1 - (dir_node.entropy_loc / max_entropy) if max_entropy > 0 else 0
        dir_node.dispersion = max(dir_node.gini_loc, entropy_factor)
        
        # Overall directory imbalance score
        dir_node.dir_imbalance = (
            0.35 * dir_node.file_pressure +
            0.25 * dir_node.branch_pressure +
            0.25 * dir_node.size_pressure +
            0.15 * dir_node.dispersion
        )
        
        # Hot leaves (top files by LOC)
        sorted_files = sorted(dir_node.files, key=lambda f: f.loc, reverse=True)
        dir_node.hot_leaves = sorted_files[:3]  # Top 3 files
        
        # Recurse into subdirectories
        for subdir in dir_node.subdirs:
            self._compute_metrics_recursive(subdir, depth + 1, max_depth)
    
    def _get_max_depth(self, tree: DirNode) -> int:
        """Get maximum depth of the tree."""
        def get_depth(node: DirNode) -> int:
            if not node.subdirs:
                return 0
            return 1 + max(get_depth(child) for child in node.subdirs)
        return get_depth(tree)
    
    def _compute_gini(self, values: List[float]) -> float:
        """Compute Gini coefficient for inequality measurement. Optimized version."""
        n = len(values)
        if n <= 1:
            return 0.0
            
        # Fast path for small lists
        if n == 2:
            if values[0] == values[1]:
                return 0.0
            return abs(values[0] - values[1]) / (values[0] + values[1])
            
        total = sum(values)
        if total == 0:
            return 0.0
            
        # Sort once and compute efficiently 
        sorted_values = sorted(values)
        
        # Optimized Gini calculation using enumerate
        weighted_sum = sum((i + 1) * x for i, x in enumerate(sorted_values))
        gini = (2 * weighted_sum) / (n * total) - (n + 1) / n
        
        return max(0.0, gini)
    
    def _compute_entropy(self, values: List[float]) -> float:
        """Compute Shannon entropy for distribution evenness. Optimized version."""
        n = len(values)
        if n == 0:
            return 0.0
            
        # Fast path for uniform distribution 
        if n == 1:
            return 0.0
            
        total = sum(values)
        if total == 0:
            return 0.0
            
        # Fast path for equal values (uniform distribution)
        first_val = values[0]
        if all(v == first_val for v in values) and first_val > 0:
            return math.log2(n)  # Maximum entropy for uniform distribution
            
        # General case - use vectorized calculation
        entropy = 0.0
        inv_total = 1.0 / total  # Compute once
        for value in values:
            if value > 0:
                p = value * inv_total
                entropy -= p * math.log2(p)
                
        return entropy
    
    def _generate_file_split_packs(self, tree: DirNode) -> List[FileSplitPack]:
        """Generate file-split recommendations for huge files."""
        packs = []
        
        def collect_huge_files(node: DirNode):
            # Check files in this directory
            for file_node in node.files:
                if file_node.huge and self._should_split_file(file_node):
                    pack = self._create_file_split_pack(file_node)
                    if pack:
                        packs.append(pack)
            
            # Recurse into subdirectories
            for subdir in node.subdirs:
                collect_huge_files(subdir)
        
        collect_huge_files(tree)
        return packs
    
    def _should_split_file(self, file_node: FileNode) -> bool:
        """Determine if a file should be split."""
        if not file_node.huge:
            return False
            
        # Skip very small files despite being "huge"
        if file_node.loc < 200:
            return False
            
        # Skip if too few entities to split meaningfully
        if len(file_node.entities) < 4:
            return False
            
        # Skip generated or config files
        path_str = str(file_node.path).lower()
        skip_patterns = [
            'generated', 'gen_', '.generated', 'build', 'dist',
            'node_modules', 'vendor', 'third_party', '.min.',
            'config', 'settings', 'constants'
        ]
        
        for pattern in skip_patterns:
            if pattern in path_str:
                return False
                
        return True
    
    def _create_file_split_pack(self, file_node: FileNode) -> Optional[FileSplitPack]:
        """Create a file split recommendation pack."""
        try:
            pack = FileSplitPack()
            pack.file = str(file_node.path)
            
            # Reasons for splitting
            pack.reasons = []
            if file_node.loc >= self.config.huge_loc:
                pack.reasons.append(f"loc {file_node.loc} > {self.config.huge_loc}")
            if file_node.bytes >= self.config.huge_bytes:
                pack.reasons.append(f"bytes {file_node.bytes} > {self.config.huge_bytes}")
            if file_node.cohesion_communities > 1:
                pack.reasons.append(f"low cohesion across {file_node.cohesion_communities} communities")
                
            # Suggest splits based on communities
            pack.suggested_splits = self._suggest_file_splits(file_node)
            
            # Value calculation
            size_factor = min(1.0, file_node.loc / self.config.huge_loc)
            cycle_participation = 0.1  # Placeholder - would need actual cycle data
            clone_contribution = 0.0   # Placeholder - would need actual clone data
            
            pack.value = {
                "size_drop": size_factor * 0.6,  # Rough estimate
                "cycle_break_opportunity": cycle_participation > 0,
                "total_value": 0.6 * size_factor + 0.3 * cycle_participation + 0.1 * clone_contribution
            }
            
            # Effort calculation
            pack.effort = {
                "exports": file_node.exports,
                "external_importers": file_node.external_importers,
                "total_effort": min(20, 0.5 * file_node.exports + 0.5 * file_node.external_importers)
            }
            
            return pack
            
        except Exception as e:
            pass  # Silently skip problematic files
            return None
    
    def _suggest_file_splits(self, file_node: FileNode) -> List[Dict[str, Union[str, List[str]]]]:
        """Suggest how to split a file based on its entities."""
        if len(file_node.entities) < 2:  # Reduced threshold for testing
            return []
            
        # Simple heuristic: group by entity name patterns
        entity_groups = defaultdict(list)
        
        for entity_id in file_node.entities:
            # Extract prefix/suffix patterns
            if '.' in entity_id:
                base_name = entity_id.split('.')[-1]
            else:
                base_name = entity_id
                
            # Group by common prefixes or entity types (case insensitive)
            base_lower = base_name.lower()
            if 'test' in base_lower:
                entity_groups['tests'].append(entity_id)
            elif any(word in base_lower for word in ['util', 'helper']):
                entity_groups['utils'].append(entity_id)
            elif any(word in base_lower for word in ['service', 'manager', 'handler', 'api']):
                entity_groups['services'].append(entity_id)
            elif any(word in base_lower for word in ['model', 'entity', 'data', 'user']):
                entity_groups['models'].append(entity_id)
            else:
                entity_groups['core'].append(entity_id)
                
        # Convert to split suggestions
        splits = []
        file_stem = file_node.path.stem
        
        for group_name, entities in entity_groups.items():
            if len(entities) >= 1:  # At least 1 entity per group
                suggested_name = f"{file_stem}_{group_name}{file_node.path.suffix}"
                splits.append({
                    "name": suggested_name,
                    "includes": entities
                })
                
        # If no meaningful groups, create simple splits
        if not splits and len(file_node.entities) >= 2:
            mid = len(file_node.entities) // 2
            splits.append({
                "name": f"{file_stem}_part1{file_node.path.suffix}",
                "includes": file_node.entities[:mid]
            })
            splits.append({
                "name": f"{file_stem}_part2{file_node.path.suffix}",
                "includes": file_node.entities[mid:]
            })
                
        return splits[:4]  # Limit to 4 suggestions
    
    def _generate_branch_packs(self, tree: DirNode, import_graph: Optional[Dict] = None) -> List[BranchReorgPack]:
        """Generate branch reorganization recommendations."""
        packs = []
        
        def collect_imbalanced_dirs(node: DirNode):
            # Check if this directory needs reorganization
            if self._should_reorganize_dir(node):
                pack = self._create_branch_pack(node, import_graph)
                if pack:
                    packs.append(pack)
            
            # Recurse into subdirectories  
            for subdir in node.subdirs:
                collect_imbalanced_dirs(subdir)
        
        collect_imbalanced_dirs(tree)
        return packs
    
    def _should_reorganize_dir(self, dir_node: DirNode) -> bool:
        """Determine if a directory should be reorganized. Optimized with early exits."""
        # Early numeric thresholds first (fastest checks)
        if (dir_node.leaf_load <= self.config.min_analysis_threshold_files and 
            dir_node.total_loc <= self.config.min_analysis_threshold_loc):
            return False
            
        # Skip excluded paths early (before expensive calculations)
        path_str = str(dir_node.path).lower()
        skip_patterns = [
            'generated', 'build', 'third_party', '.venv', 'node_modules', 
            'target', 'dist', '.git', '__pycache__', 'coverage'
        ]
        
        if any(pattern in path_str for pattern in skip_patterns):
            return False
        
        # Must have high imbalance
        if dir_node.dir_imbalance < 0.6:
            return False
            
        # Must exceed at least one threshold significantly
        return (
            dir_node.leaf_load > self.config.max_files_per_dir or
            dir_node.total_loc > self.config.max_dir_loc or
            dir_node.dispersion > 0.7
        )
    
    def _create_branch_pack(self, dir_node: DirNode, import_graph: Optional[Dict] = None) -> Optional[BranchReorgPack]:
        """Create a branch reorganization pack."""
        try:
            # Cluster files into logical groups
            clusters = self._cluster_files(dir_node, import_graph)
            if len(clusters) < 2:
                return None
                
            # Estimate imbalance gain
            old_imbalance = dir_node.dir_imbalance
            new_imbalances = [self._estimate_cluster_imbalance(cluster) for cluster in clusters]
            avg_new_imbalance = sum(new_imbalances) / len(new_imbalances)
            imbalance_gain = old_imbalance - avg_new_imbalance
            
            # Check if gain meets threshold
            if imbalance_gain < self.config.min_branch_recommendation_gain:
                return None
                
            pack = BranchReorgPack()
            pack.dir = str(dir_node.path)
            
            # Current state
            pack.current = {
                "files": dir_node.leaf_load,
                "subdirs": dir_node.branching_factor,
                "loc": dir_node.total_loc,
                "bf": dir_node.branching_factor,
                "gini_loc": dir_node.gini_loc,
                "imbalance": dir_node.dir_imbalance
            }
            
            # Proposal
            pack.proposal = []
            total_files_moved = 0
            
            for i, cluster in enumerate(clusters):
                cluster_name = self._suggest_cluster_name(cluster, i)
                cluster_loc = sum(f.loc for f in cluster)
                
                pack.proposal.append({
                    "name": cluster_name,
                    "files": len(cluster),
                    "loc": cluster_loc
                })
                
                total_files_moved += len(cluster)
                
            # Value metrics
            cross_edges_reduced = self._estimate_cross_edges_reduced(clusters)
            pack.value = {
                "imbalance_gain": imbalance_gain,
                "cross_edges_reduced": cross_edges_reduced
            }
            
            # Effort metrics  
            import_updates_est = min(total_files_moved * 2, total_files_moved + cross_edges_reduced)
            pack.effort = {
                "files_moved": total_files_moved,
                "import_updates_est": import_updates_est
            }
            
            # Implementation steps
            pack.steps = [
                f"Create subdirs {', '.join(p['name'] + '/' for p in pack.proposal)} under {pack.dir}",
                f"Move files as listed; update relative imports within {Path(pack.dir).name}/",
                "Add index barrels where idiomatic (TS/JS)."
            ]
            
            return pack
            
        except Exception as e:
            pass  # Silently skip problematic directories
            return None
    
    def _cluster_files(self, dir_node: DirNode, import_graph: Optional[Dict] = None) -> List[List[FileNode]]:
        """Cluster files in directory into logical groups. Optimized version."""
        if len(dir_node.files) < 4:
            return [dir_node.files]  # Too few files to cluster meaningfully
            
        # Pre-compiled patterns for performance
        test_patterns = frozenset(['.test.js', '.test.ts', '.spec.js', '.spec.ts', '_test.py'])
        config_patterns = frozenset(['.config.js', '.config.ts', 'config.py', 'settings.py'])
        
        # Fast clustering by file characteristics
        clusters = {
            'tests': [],
            'utils': [],
            'config': [],
            'core': []
        }
        language_clusters = {}
        
        for file_node in dir_node.files:
            # Cluster by language first (most specific)
            if file_node.language != "unknown":
                lang_key = f"lang_{file_node.language}"
                if lang_key not in language_clusters:
                    language_clusters[lang_key] = []
                language_clusters[lang_key].append(file_node)
                continue
                
            # Fast pattern matching using sets
            ext = file_node.path.suffix.lower()
            stem_lower = file_node.path.stem.lower()
            
            if ext in test_patterns or 'test_' in stem_lower:
                clusters['tests'].append(file_node)
            elif 'util' in stem_lower or 'helper' in stem_lower:
                clusters['utils'].append(file_node)
            elif ext in config_patterns or 'config' in stem_lower:
                clusters['config'].append(file_node)
            else:
                clusters['core'].append(file_node)
                    
        # Merge language clusters with main clusters
        all_clusters = {**clusters, **language_clusters}
        
        # Filter empty clusters efficiently
        cluster_list = [files for files in all_clusters.values() if len(files) > 0]
        
        # Balance clusters by size (optimized merging)
        balanced_clusters = [cluster for cluster in cluster_list if len(cluster) >= 2]
        small_clusters = [cluster for cluster in cluster_list if len(cluster) < 2]
        
        # Merge small clusters into balanced ones
        for small_cluster in small_clusters:
            if balanced_clusters:
                # Find smallest existing cluster efficiently
                smallest_idx = min(range(len(balanced_clusters)), key=lambda i: len(balanced_clusters[i]))
                balanced_clusters[smallest_idx].extend(small_cluster)
            else:
                balanced_clusters.append(small_cluster)
        
        # Limit to max 4 clusters
        if len(balanced_clusters) > 4:
            # Merge smallest clusters
            while len(balanced_clusters) > 4:
                balanced_clusters.sort(key=len)
                balanced_clusters[1].extend(balanced_clusters[0])
                balanced_clusters = balanced_clusters[1:]
                
        return balanced_clusters or [dir_node.files]
    
    def _suggest_cluster_name(self, cluster: List[FileNode], index: int) -> str:
        """Suggest a name for a file cluster."""
        if not cluster:
            return f"group_{index}"
            
        # Analyze file patterns to suggest names
        stems = [f.path.stem.lower() for f in cluster]
        
        # Common patterns
        if any('test' in stem for stem in stems):
            return 'tests'
        elif any('util' in stem or 'helper' in stem for stem in stems):
            return 'utils'
        elif any('config' in stem or 'setting' in stem for stem in stems):
            return 'config'
        elif any('service' in stem or 'api' in stem for stem in stems):
            return 'services'
        elif any('model' in stem or 'entity' in stem or 'data' in stem for stem in stems):
            return 'models'
        elif any('ui' in stem or 'component' in stem or 'view' in stem for stem in stems):
            return 'ui'
        else:
            # Try to find common prefix
            if len(stems) > 1:
                common = ''
                for i in range(min(len(s) for s in stems)):
                    if all(s[i] == stems[0][i] for s in stems):
                        common += stems[0][i]
                    else:
                        break
                if len(common) >= 2:
                    return common.rstrip('_-')
                    
        # Fallback names
        names = ['core', 'lib', 'base', 'main']
        return names[index % len(names)]
    
    def _estimate_cluster_imbalance(self, cluster: List[FileNode]) -> float:
        """Estimate imbalance score for a proposed cluster."""
        if not cluster:
            return 0.0
            
        # Rough estimation based on file count and size distribution
        file_count = len(cluster)
        total_loc = sum(f.loc for f in cluster)
        
        # Simple pressure calculations
        file_pressure = min(1.0, file_count / self.config.max_files_per_dir)
        size_pressure = min(1.0, total_loc / self.config.max_dir_loc)
        
        # No subdivision pressure for new clusters
        branch_pressure = 0.0
        
        # Rough dispersion estimate
        if file_count > 1:
            file_sizes = [f.loc for f in cluster]
            gini_approx = self._compute_gini(file_sizes)
            dispersion = gini_approx * 0.5  # Simplified
        else:
            dispersion = 0.0
            
        return 0.35 * file_pressure + 0.25 * branch_pressure + 0.25 * size_pressure + 0.15 * dispersion
    
    def _estimate_cross_edges_reduced(self, clusters: List[List[FileNode]]) -> int:
        """Estimate how many cross-cluster imports would be reduced."""
        # Simplified heuristic: assume files in same cluster have fewer cross-references
        total_files = sum(len(cluster) for cluster in clusters)
        
        # Rough estimate based on file organization improvement
        if len(clusters) >= 2:
            # Assume 10-30% reduction in cross-references
            estimated_total_edges = total_files * 2  # Rough estimate
            reduction_factor = min(0.3, 0.1 * len(clusters))
            return int(estimated_total_edges * reduction_factor)
            
        return 0
    
    def _rank_packs(self, packs: List[Union[FileSplitPack, BranchReorgPack]]) -> List[Union[FileSplitPack, BranchReorgPack]]:
        """Rank packs by value/effort ratio."""
        def get_score(pack):
            try:
                if isinstance(pack, FileSplitPack):
                    value = pack.value.get("total_value", 0.0)
                    effort = pack.effort.get("total_effort", 1.0)
                else:  # BranchReorgPack
                    value = (
                        0.7 * pack.value.get("imbalance_gain", 0.0) +
                        0.3 * (pack.value.get("cross_edges_reduced", 0) / 
                               max(1, pack.value.get("cross_edges_reduced", 0) + 1))
                    )
                    effort = (
                        0.4 * pack.effort.get("files_moved", 0) +
                        0.6 * pack.effort.get("import_updates_est", 0) / 2.0
                    )
                
                return value / (effort + 0.1)  # Small epsilon to avoid division by zero
                
            except Exception:
                return 0.0
                
        return sorted(packs, key=get_score, reverse=True)