"""
Main pipeline orchestration for valknut analysis.
"""

import asyncio
import logging
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set
from uuid import UUID, uuid4

import networkx as nx

from valknut.core.config import RefactorRankConfig
from valknut.core.errors import RefactorRankError, LanguageNotSupportedError
from valknut.core.featureset import FeatureVector, feature_registry
from valknut.core.registry import language_registry
from valknut.core.scoring import FeatureNormalizer, RankingSystem
from valknut.detectors.complexity import ComplexityExtractor
from valknut.detectors.graph import GraphExtractor
from valknut.detectors.echo_bridge import create_echo_extractor
from valknut.io.cache import CacheManager
from valknut.io.fsrepo import FileDiscovery
from valknut.lang.common_ast import Entity, ParseIndex
from valknut.core.impact_packs import ImpactPackBuilder, ImpactPack

logger = logging.getLogger(__name__)


@dataclass
class PipelineResult:
    """Result of pipeline analysis."""
    
    result_id: UUID
    config: RefactorRankConfig
    total_files: int
    total_entities: int
    processing_time: float
    feature_vectors: List[FeatureVector] = field(default_factory=list)
    ranked_entities: List[tuple[FeatureVector, float]] = field(default_factory=list)
    impact_packs: List[ImpactPack] = field(default_factory=list)
    errors: List[str] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    
    @property
    def top_k_entities(self) -> List[tuple[FeatureVector, float]]:
        """Get top-k entities based on config."""
        return self.ranked_entities[:self.config.ranking.top_k]


class Pipeline:
    """Main analysis pipeline."""
    
    def __init__(self, config: RefactorRankConfig) -> None:
        self.config = config
        self.cache_manager = CacheManager(config.cache_dir, config.cache_ttl)
        self.file_discovery = FileDiscovery()
        
        # Initialize feature extractors
        self._initialize_extractors()
        
        # Initialize components
        self.normalizer = FeatureNormalizer(config.normalize)
        self.ranking_system = RankingSystem(config.weights)
        self.impact_pack_builder = ImpactPackBuilder(
            enable_clone_packs=True,  # Always enable clone packs since we have clone groups
            enable_cycle_packs=config.impact_packs.enable_cycle_packs,
            enable_chokepoint_packs=config.impact_packs.enable_chokepoint_packs,
            max_packs=config.impact_packs.max_packs,
            non_overlap=config.impact_packs.non_overlap,
            centrality_samples=config.impact_packs.centrality_samples,
            min_similarity=config.detectors.echo.min_similarity,
            min_total_loc=config.clone.min_total_loc,
        )
    
    def _initialize_extractors(self) -> None:
        """Initialize and register feature extractors."""
        # Register built-in extractors
        feature_registry.register(ComplexityExtractor())
        feature_registry.register(GraphExtractor())
        
        # Register echo extractor if enabled
        if self.config.detectors.echo.enabled:
            echo_extractor = create_echo_extractor(
                min_similarity=self.config.detectors.echo.min_similarity,
                min_tokens=self.config.detectors.echo.min_tokens,
            )
            feature_registry.register(echo_extractor)
    
    async def analyze(self) -> PipelineResult:
        """
        Run the complete analysis pipeline.
        
        Returns:
            Pipeline result with ranked entities
        """
        start_time = time.time()
        result_id = uuid4()
        
        logger.info(f"Starting pipeline analysis {result_id}")
        
        try:
            # Stage 1: Discover files
            logger.info("Stage 1: Discovering files")
            discovered_files = await self._discover_files()
            logger.info(f"Discovered {len(discovered_files)} files")
            
            # Stage 2: Parse and index files
            logger.info("Stage 2: Parsing and indexing")
            parse_indices = await self._parse_and_index(discovered_files)
            
            # Count total entities
            total_entities = sum(len(index.entities) for index in parse_indices.values())
            logger.info(f"Parsed {total_entities} entities")
            
            # Stage 3: Extract features
            logger.info("Stage 3: Extracting features")
            feature_vectors = await self._extract_features(parse_indices)
            logger.info(f"Extracted features for {len(feature_vectors)} entities")
            
            # Stage 4: Validate and normalize features
            logger.info("Stage 4: Validating feature variance")
            self._validate_feature_variance(feature_vectors)
            
            logger.info("Stage 4b: Normalizing features")
            normalized_vectors = self._normalize_features(feature_vectors)
            
            # Stage 5: Score and rank
            logger.info("Stage 5: Scoring and ranking")
            ranked_entities = self.ranking_system.rank_entities(
                normalized_vectors, 
                top_k=self.config.ranking.top_k
            )
            
            # Stage 6: Generate impact packs
            logger.info("Stage 6: Generating impact packs")
            impact_packs = await self._generate_impact_packs(parse_indices)
            logger.info(f"Generated {len(impact_packs)} impact packs")
            
            processing_time = time.time() - start_time
            
            result = PipelineResult(
                result_id=result_id,
                config=self.config,
                total_files=len(discovered_files),
                total_entities=total_entities,
                processing_time=processing_time,
                feature_vectors=normalized_vectors,
                ranked_entities=ranked_entities,
                impact_packs=impact_packs,
            )
            
            logger.info(
                f"Pipeline completed in {processing_time:.2f}s. "
                f"Top entity score: {ranked_entities[0][1]:.3f}" if ranked_entities else "No entities ranked"
            )
            
            return result
            
        except Exception as e:
            logger.error(f"Pipeline analysis failed: {e}")
            raise RefactorRankError(f"Pipeline analysis failed: {e}") from e
    
    async def _discover_files(self) -> List[Path]:
        """Discover files to analyze."""
        all_files = []
        
        for root_config in self.config.roots:
            try:
                files = self.file_discovery.discover(
                    roots=[root_config.path],
                    include_patterns=root_config.include,
                    exclude_patterns=root_config.exclude,
                    languages=self.config.languages,
                )
                all_files.extend(files)
                logger.debug(f"Found {len(files)} files in {root_config.path}")
                
            except Exception as e:
                logger.warning(f"Failed to discover files in {root_config.path}: {e}")
        
        # Remove duplicates while preserving order
        seen = set()
        unique_files = []
        for file_path in all_files:
            if file_path not in seen:
                seen.add(file_path)
                unique_files.append(file_path)
        
        return unique_files
    
    async def _parse_and_index(self, files: List[Path]) -> Dict[str, ParseIndex]:
        """Parse files and build indices by language."""
        indices_by_language: Dict[str, ParseIndex] = {}
        
        # Group files by language
        files_by_language: Dict[str, List[Path]] = {}
        
        for file_path in files:
            # Determine language from extension
            adapter = language_registry.get_adapter_by_extension(file_path.suffix)
            if adapter:
                language = adapter.language
                if language not in files_by_language:
                    files_by_language[language] = []
                files_by_language[language].append(file_path)
        
        # Parse each language group
        for language, language_files in files_by_language.items():
            if language not in self.config.languages:
                logger.debug(f"Skipping {language} files (not in config)")
                continue
            
            try:
                adapter = language_registry.get_adapter(language)
                
                # Check cache first
                cache_key = self._get_cache_key(language, language_files)
                cached_index = await self.cache_manager.get_parse_index(cache_key)
                
                if cached_index:
                    logger.debug(f"Using cached index for {language}")
                    indices_by_language[language] = cached_index
                else:
                    logger.debug(f"Parsing {len(language_files)} {language} files")
                    print(f"🔍 DEBUG: Pipeline calling adapter.parse_index on {adapter.__class__.__name__}")
                    print(f"🔍 DEBUG: Adapter methods: {[m for m in dir(adapter) if not m.startswith('_')]}")
                    index = adapter.parse_index(language_files)
                    
                    # Check what we got back
                    if hasattr(index, 'entities'):
                        entity_count = len(index.entities)
                        print(f"🔍 DEBUG: Got {entity_count} entities from adapter")
                        if entity_count > 0:
                            first_entity = next(iter(index.entities.values()))
                            print(f"🔍 DEBUG: First entity: {first_entity.name}, raw_text={'PRESENT' if first_entity.raw_text else 'MISSING'}")
                    
                    indices_by_language[language] = index
                    
                    # Cache the result
                    await self.cache_manager.set_parse_index(cache_key, index)
                
            except LanguageNotSupportedError:
                logger.warning(f"Language {language} not supported, skipping")
            except Exception as e:
                logger.error(f"Failed to parse {language} files: {e}")
        
        return indices_by_language
    
    async def _extract_features(self, indices: Dict[str, ParseIndex]) -> List[FeatureVector]:
        """Extract features from all entities."""
        all_feature_vectors = []
        
        # Process each language index
        for language, index in indices.items():
            logger.debug(f"Extracting features for {language}")
            
            # Get entities based on granularity
            entities = self._get_entities_by_granularity(index, language)
            
            # Extract features for each entity
            for entity in entities:
                try:
                    feature_vector = feature_registry.extract_all_features(entity, index)
                    all_feature_vectors.append(feature_vector)
                    
                except Exception as e:
                    logger.warning(f"Failed to extract features for {entity.id}: {e}")
        
        # Debug logging for feature extraction results
        if all_feature_vectors:
            self._log_feature_extraction_stats(all_feature_vectors)
        
        return all_feature_vectors
    
    def _log_feature_extraction_stats(self, feature_vectors: List[FeatureVector]) -> None:
        """Log feature extraction statistics to identify flat features."""
        logger.info("=== FEATURE EXTRACTION DIAGNOSTIC ===")
        
        # Collect all feature values by feature name
        feature_values = {}
        for vector in feature_vectors:
            for feature_name, value in vector.features.items():
                if feature_name not in feature_values:
                    feature_values[feature_name] = []
                feature_values[feature_name].append(value)
        
        flat_features = []
        varied_features = []
        
        for feature_name, values in feature_values.items():
            if not values:
                continue
                
            min_val = min(values)
            max_val = max(values)
            unique_count = len(set(values))
            total_count = len(values)
            
            logger.info(f"  {feature_name}: min={min_val:.3f}, max={max_val:.3f}, unique={unique_count}/{total_count}")
            
            if min_val == max_val:
                flat_features.append(feature_name)
                logger.warning(f"    ⚠️  FLAT FEATURE: All {total_count} values are {min_val}")
            elif unique_count == 1:
                flat_features.append(feature_name)
                logger.warning(f"    ⚠️  FLAT FEATURE: Only one unique value ({min_val})")
            elif unique_count <= 2:
                logger.warning(f"    ⚠️  LOW VARIANCE: Only {unique_count} unique values")
            else:
                varied_features.append(feature_name)
        
        if flat_features:
            logger.error(f"🚨 FOUND {len(flat_features)} FLAT FEATURES: {flat_features}")
            logger.error("These will trigger the '0.5 everywhere' normalization bug!")
            logger.error("Possible causes:")
            logger.error("  1. All test entities have identical complexity")
            logger.error("  2. Feature extractors are not working correctly")
            logger.error("  3. Granularity setting extracts only one type of entity")
            logger.error("  4. Test data lacks sufficient code variety")
        else:
            logger.info(f"✅ All {len(varied_features)} features have variance - normalization should work")
    
    def _validate_feature_variance(self, feature_vectors: List[FeatureVector]) -> None:
        """Validate feature variance and suggest fixes for flat features."""
        if not feature_vectors:
            logger.warning("No feature vectors to validate")
            return
        
        # Collect feature statistics
        feature_stats = {}
        for vector in feature_vectors:
            for feature_name, value in vector.features.items():
                if feature_name not in feature_stats:
                    feature_stats[feature_name] = []
                feature_stats[feature_name].append(value)
        
        flat_features = []
        low_variance_features = []
        
        for feature_name, values in feature_stats.items():
            if not values:
                continue
                
            unique_count = len(set(values))
            total_count = len(values)
            variance_ratio = unique_count / total_count
            
            if unique_count == 1:
                flat_features.append(feature_name)
            elif variance_ratio < 0.3:  # Less than 30% unique values
                low_variance_features.append(feature_name)
        
        # Report and suggest fixes
        if flat_features or low_variance_features:
            logger.warning("=== FEATURE VARIANCE VALIDATION ===")
            
            if flat_features:
                logger.error(f"🚨 FLAT FEATURES DETECTED: {flat_features}")
                logger.error("These will cause '0.5 everywhere' normalization bug!")
                self._suggest_flat_feature_fixes(flat_features, feature_vectors)
            
            if low_variance_features:
                logger.warning(f"⚠️  LOW VARIANCE FEATURES: {low_variance_features}")
                logger.warning("These may cause poor ranking discrimination")
        else:
            logger.info("✅ Feature variance validation passed")
    
    def _suggest_flat_feature_fixes(self, flat_features: List[str], feature_vectors: List[FeatureVector]) -> None:
        """Suggest specific fixes for flat features."""
        logger.info("🔧 SUGGESTED FIXES:")
        
        # Analyze the data to suggest specific fixes
        entity_types = set()
        granularity = self.config.ranking.granularity
        
        for vector in feature_vectors[:5]:  # Sample first 5 vectors
            # Extract entity type from metadata if available
            entity_type = vector.metadata.get('entity_kind', 'unknown')
            entity_types.add(entity_type)
        
        logger.info(f"  📊 Current analysis: {len(feature_vectors)} entities, granularity='{granularity}'")
        logger.info(f"  📊 Entity types found: {entity_types}")
        
        # Specific suggestions based on flat features
        if 'cyclomatic' in flat_features or 'cognitive' in flat_features:
            logger.info("  💡 Complexity features are flat:")
            logger.info("     - Try analyzing files with more varied function complexity")
            logger.info("     - Check if granularity='function' to get function-level complexity")
            logger.info("     - Verify test data includes functions with loops, conditionals")
        
        if 'fan_in' in flat_features or 'fan_out' in flat_features:
            logger.info("  💡 Graph features are flat:")
            logger.info("     - Ensure test data has import/call relationships")
            logger.info("     - Try granularity='file' for import graph analysis")
            logger.info("     - Check if multiple files are being analyzed together")
        
        if 'clone_mass' in flat_features:
            logger.info("  💡 Clone features are flat:")
            logger.info("     - Verify echo detector is enabled in config")
            logger.info("     - Check if test data has sufficient code duplication")
            logger.info("     - Ensure min_similarity threshold allows detection")
        
        # Generic suggestions
        logger.info("  🎯 General recommendations:")
        logger.info(f"     - Switch to 'minmax' normalization to avoid 0.5 fallbacks")
        logger.info(f"     - Try different granularity: {['auto', 'file', 'function', 'class']}")
        logger.info(f"     - Analyze more diverse code samples")
        logger.info(f"     - Check feature extractor implementations")
    
    def _get_entities_by_granularity(self, index: ParseIndex, language: str) -> List[Entity]:
        """Get entities based on configured granularity."""
        granularity = self.config.ranking.granularity
        
        if granularity == "auto":
            # Auto-select based on language
            if language in ["python", "typescript", "javascript"]:
                granularity = "function"
            else:
                granularity = "file"
        
        if granularity == "file":
            return index.get_by_kind(EntityKind.FILE)
        elif granularity == "function":
            return (index.get_by_kind(EntityKind.FUNCTION) + 
                    index.get_by_kind(EntityKind.METHOD))
        elif granularity == "class":
            return index.get_by_kind(EntityKind.CLASS)
        else:
            # Return all entities
            return list(index.entities.values())
    
    def _normalize_features(self, feature_vectors: List[FeatureVector]) -> List[FeatureVector]:
        """Normalize feature vectors."""
        if not feature_vectors:
            return []
        
        # Fit normalizer to all feature vectors
        self.normalizer.fit(feature_vectors)
        
        # Normalize each vector
        normalized_vectors = []
        for vector in feature_vectors:
            normalized = self.normalizer.normalize(vector)
            normalized_vectors.append(normalized)
        
        return normalized_vectors
    
    def _get_cache_key(self, language: str, files: List[Path]) -> str:
        """Generate cache key for a set of files."""
        # Simple hash based on language and file paths/mtimes
        import hashlib
        
        hash_input = f"{language}:"
        for file_path in sorted(files):
            try:
                mtime = file_path.stat().st_mtime
                hash_input += f"{file_path}:{mtime}:"
            except OSError:
                hash_input += f"{file_path}:0:"
        
        return hashlib.sha256(hash_input.encode()).hexdigest()
    
    async def _generate_impact_packs(self, parse_indices: Dict[str, ParseIndex]) -> List[ImpactPack]:
        """Generate impact packs from parse indices."""
        # Combine all entities from all languages
        all_entities = {}
        combined_index = None
        
        for language, index in parse_indices.items():
            all_entities.update(index.entities)
            if combined_index is None:
                combined_index = index
            else:
                # Merge import and call graphs
                combined_index.import_graph = nx.compose(combined_index.import_graph, index.import_graph)
                combined_index.call_graph = nx.compose(combined_index.call_graph, index.call_graph)
        
        if not combined_index:
            return []
        
        # Get clone groups from echo extractor
        clone_groups = await self._get_clone_groups(parse_indices)
        
        # Build impact packs
        impact_packs = self.impact_pack_builder.build_all_packs(
            combined_index, 
            clone_groups, 
            all_entities
        )
        
        return impact_packs
    
    async def _get_clone_groups(self, parse_indices: Dict[str, ParseIndex]) -> List[Dict]:
        """Get clone groups from echo extractor if available."""
        if not self.config.detectors.echo.enabled:
            return []
        
        # This is a simplified implementation - real echo integration would provide clone groups
        # For now, return empty list since echo_bridge handles the actual clone detection
        clone_groups = []
        
        # In a real implementation, we would:
        # 1. Collect all source code from entities
        # 2. Run echo analysis to get clone groups
        # 3. Return formatted clone groups
        
        return clone_groups


# Global result storage for server use
_results_cache: Dict[UUID, PipelineResult] = {}


async def analyze(config: RefactorRankConfig) -> PipelineResult:
    """
    Analyze a codebase with the given configuration.
    
    Args:
        config: Analysis configuration
        
    Returns:
        Analysis results
    """
    pipeline = Pipeline(config)
    result = await pipeline.analyze()
    
    # Store result in global cache for server use
    _results_cache[result.result_id] = result
    
    return result


def get_result(result_id: UUID) -> Optional[PipelineResult]:
    """Get analysis result by ID."""
    return _results_cache.get(result_id)


def clear_results() -> None:
    """Clear all cached results."""
    _results_cache.clear()


# Import statements for entity kind
from valknut.lang.common_ast import EntityKind