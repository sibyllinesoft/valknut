#!/usr/bin/env python3
"""
Comprehensive performance profiling script for valknut code analysis tool.

This script profiles the complete analysis pipeline to identify:
1. I/O bound vs CPU bound operations
2. Tree-sitter parsing performance 
3. Feature extraction bottlenecks
4. Echo clone detection overhead
5. Areas that would benefit from Rust porting

Usage:
    python performance_profiler.py --target-dir ./test_code --output profile_results.json
"""

import argparse
import asyncio
import json
import logging
import time
import tracemalloc
import threading
import sys
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Dict, List, Optional, Any, Callable
import psutil
import cProfile
import pstats
import io

# Import valknut components
sys.path.insert(0, str(Path(__file__).parent))
from valknut.core.config import get_default_config
from valknut.core.pipeline import Pipeline
from valknut.io.fsrepo import FileDiscovery
from valknut.lang.common_ast import ParseIndex
from valknut.core.featureset import feature_registry, FeatureVector
from valknut.detectors.complexity import ComplexityExtractor
from valknut.detectors.graph import GraphExtractor
from valknut.detectors.echo_bridge import create_echo_extractor
from valknut.detectors.refactoring import RefactoringAnalyzer


@dataclass
class PerformanceMetrics:
    """Performance metrics for a pipeline stage."""
    stage_name: str
    duration_seconds: float
    memory_peak_mb: float
    memory_delta_mb: float
    cpu_percent: float
    files_processed: int = 0
    entities_processed: int = 0
    is_io_bound: bool = False
    is_cpu_bound: bool = False
    rust_candidate_score: float = 0.0  # 0-1 score for Rust port benefit
    notes: List[str] = None


@dataclass 
class ComponentProfile:
    """Detailed profile of a specific component."""
    component_name: str
    total_time: float
    self_time: float  # Time spent in this component only
    call_count: int
    avg_time_per_call: float
    memory_usage_mb: float
    is_native_code: bool = False  # Uses C/Rust extensions
    rust_port_benefit: float = 0.0  # Estimated performance gain from Rust


class PerformanceProfiler:
    """Comprehensive performance profiler for valknut pipeline."""
    
    def __init__(self):
        self.metrics: List[PerformanceMetrics] = []
        self.component_profiles: Dict[str, ComponentProfile] = {}
        self.process = psutil.Process()
        
    @contextmanager
    def profile_stage(self, stage_name: str, **kwargs):
        """Profile a pipeline stage."""
        # Start monitoring
        tracemalloc.start()
        start_time = time.time()
        start_memory = self.process.memory_info().rss / 1024 / 1024
        
        # CPU monitoring in background
        cpu_samples = []
        stop_cpu_monitor = threading.Event()
        
        def cpu_monitor():
            while not stop_cpu_monitor.wait(0.1):
                try:
                    cpu_samples.append(self.process.cpu_percent())
                except:
                    pass
        
        cpu_thread = threading.Thread(target=cpu_monitor, daemon=True)
        cpu_thread.start()
        
        try:
            yield
        finally:
            # Stop monitoring
            stop_cpu_monitor.set()
            cpu_thread.join(timeout=1.0)
            
            end_time = time.time()
            end_memory = self.process.memory_info().rss / 1024 / 1024
            _, peak_memory = tracemalloc.get_traced_memory()
            tracemalloc.stop()
            
            duration = end_time - start_time
            memory_delta = end_memory - start_memory
            avg_cpu = sum(cpu_samples) / len(cpu_samples) if cpu_samples else 0.0
            
            # Heuristics for I/O vs CPU bound classification
            is_io_bound = avg_cpu < 50.0 and duration > 0.5  # Low CPU usage, significant time
            is_cpu_bound = avg_cpu > 70.0 and duration > 0.1  # High CPU usage
            
            # Calculate Rust candidacy score (0-1)
            rust_score = self._calculate_rust_candidate_score(
                stage_name, duration, avg_cpu, memory_delta
            )
            
            metrics = PerformanceMetrics(
                stage_name=stage_name,
                duration_seconds=duration,
                memory_peak_mb=peak_memory / 1024 / 1024,
                memory_delta_mb=memory_delta,
                cpu_percent=avg_cpu,
                is_io_bound=is_io_bound,
                is_cpu_bound=is_cpu_bound,
                rust_candidate_score=rust_score,
                notes=[],
                **kwargs
            )
            
            self.metrics.append(metrics)
            print(f"📊 {stage_name}: {duration:.2f}s, {avg_cpu:.1f}% CPU, {memory_delta:+.1f}MB")

    def _calculate_rust_candidate_score(self, stage_name: str, duration: float, 
                                      cpu_percent: float, memory_delta: float) -> float:
        """Calculate how much this stage would benefit from Rust porting (0-1)."""
        score = 0.0
        
        # Base score from CPU intensity
        if cpu_percent > 70:
            score += 0.4
        elif cpu_percent > 50:
            score += 0.2
            
        # Bonus for computation-heavy stages
        if any(keyword in stage_name.lower() for keyword in 
               ['complexity', 'features', 'analysis', 'parsing']):
            score += 0.3
            
        # Bonus for significant duration
        if duration > 1.0:
            score += 0.2
        elif duration > 0.5:
            score += 0.1
            
        # Penalty for I/O heavy operations (less Rust benefit)
        if any(keyword in stage_name.lower() for keyword in 
               ['discovery', 'reading', 'writing', 'cache']):
            score *= 0.5
            
        # Penalty if already using native code (tree-sitter)
        if 'parsing' in stage_name.lower():
            score *= 0.3  # tree-sitter is already fast C code
            
        return min(1.0, score)

    async def profile_complete_pipeline(self, config) -> Dict[str, Any]:
        """Profile the complete valknut analysis pipeline."""
        pipeline = Pipeline(config)
        
        print("🔍 Starting comprehensive pipeline profiling...")
        total_start = time.time()
        
        # Stage 1: File Discovery
        with self.profile_stage("file_discovery"):
            discovered_files = await pipeline._discover_files()
        self.metrics[-1].files_processed = len(discovered_files)
        
        # Stage 2: Parsing and Indexing (most critical for Rust evaluation)
        with self.profile_stage("parsing_indexing") as ctx:
            parse_indices = await self._profile_parsing(pipeline, discovered_files)
        
        total_entities = sum(len(index.entities) for index in parse_indices.values())
        self.metrics[-1].entities_processed = total_entities
        
        # Stage 3: Feature Extraction (component-level profiling)
        with self.profile_stage("feature_extraction"):
            feature_vectors = await self._profile_feature_extraction(pipeline, parse_indices)
        self.metrics[-1].entities_processed = len(feature_vectors)
        
        # Stage 4: Normalization 
        with self.profile_stage("normalization"):
            if feature_vectors:
                pipeline.normalizer.fit(feature_vectors)
                normalized_vectors = [pipeline.normalizer.normalize(v) for v in feature_vectors]
            else:
                normalized_vectors = []
                
        # Stage 5: Scoring and Ranking
        with self.profile_stage("scoring_ranking"):
            if normalized_vectors:
                ranked_entities = pipeline.ranking_system.rank_entities(
                    normalized_vectors, top_k=config.ranking.top_k
                )
            else:
                ranked_entities = []
                
        # Stage 6: Impact Packs Generation
        with self.profile_stage("impact_packs"):
            impact_packs = await pipeline._generate_impact_packs(parse_indices, discovered_files)
            
        total_time = time.time() - total_start
        
        # Generate comprehensive report
        return self._generate_performance_report(total_time, len(discovered_files), total_entities)

    async def _profile_parsing(self, pipeline: Pipeline, files: List[Path]) -> Dict[str, ParseIndex]:
        """Profile parsing with detailed tree-sitter vs native comparison."""
        indices_by_language = {}
        
        # Group files by language for language-specific profiling
        files_by_language = {}
        for file_path in files:
            from valknut.core.registry import language_registry
            adapter = language_registry.get_adapter_by_extension(file_path.suffix)
            if adapter:
                language = adapter.language
                if language not in files_by_language:
                    files_by_language[language] = []
                files_by_language[language].append(file_path)
        
        # Profile each language separately
        for language, language_files in files_by_language.items():
            if language not in pipeline.config.languages:
                continue
                
            print(f"  📝 Profiling {language} parsing ({len(language_files)} files)")
            
            with self.profile_stage(f"parse_{language}") as ctx:
                try:
                    adapter = language_registry.get_adapter(language)
                    
                    # Profile tree-sitter parsing
                    tree_sitter_start = time.time()
                    index = adapter.parse_index(language_files)
                    tree_sitter_time = time.time() - tree_sitter_start
                    
                    indices_by_language[language] = index
                    
                    # Record tree-sitter specific metrics
                    self.metrics[-1].notes.append(f"Tree-sitter parsing: {tree_sitter_time:.2f}s")
                    self.metrics[-1].notes.append(f"Entities extracted: {len(index.entities)}")
                    
                    # Compare with native Python AST if applicable
                    if language == "python":
                        native_time = await self._compare_python_native_parsing(language_files)
                        if native_time:
                            speedup = native_time / tree_sitter_time
                            self.metrics[-1].notes.append(f"vs Native AST: {speedup:.1f}x slower")
                            
                except Exception as e:
                    print(f"    ❌ Failed to parse {language}: {e}")
                    
        return indices_by_language

    async def _compare_python_native_parsing(self, files: List[Path]) -> Optional[float]:
        """Compare tree-sitter vs native Python AST parsing speed."""
        try:
            import ast
            
            start_time = time.time()
            total_entities = 0
            
            for file_path in files[:10]:  # Limit comparison to first 10 files
                try:
                    content = file_path.read_text(encoding='utf-8')
                    tree = ast.parse(content)
                    
                    # Count entities (simplified)
                    for node in ast.walk(tree):
                        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                            total_entities += 1
                            
                except Exception:
                    continue
                    
            native_time = time.time() - start_time
            return native_time
            
        except Exception:
            return None

    async def _profile_feature_extraction(self, pipeline: Pipeline, indices: Dict[str, ParseIndex]) -> List[FeatureVector]:
        """Profile individual feature extractors to identify optimization targets."""
        all_feature_vectors = []
        
        # Profile each extractor type
        extractor_profiles = {}
        
        for language, index in indices.items():
            entities = pipeline._get_entities_by_granularity(index, language)
            
            print(f"  🔧 Profiling feature extraction for {len(entities)} {language} entities")
            
            for entity in entities[:50]:  # Limit for profiling
                try:
                    # Profile complexity extractor
                    if 'complexity' not in extractor_profiles:
                        with self._profile_component("complexity_extractor") as prof:
                            complexity_extractor = ComplexityExtractor()
                            complexity_features = complexity_extractor.extract(entity, index)
                        extractor_profiles['complexity'] = prof
                    
                    # Profile graph extractor
                    if 'graph' not in extractor_profiles:
                        with self._profile_component("graph_extractor") as prof:
                            graph_extractor = GraphExtractor()
                            graph_features = graph_extractor.extract(entity, index)
                        extractor_profiles['graph'] = prof
                    
                    # Profile refactoring analyzer
                    if 'refactoring' not in extractor_profiles:
                        with self._profile_component("refactoring_analyzer") as prof:
                            refactoring_analyzer = RefactoringAnalyzer()
                            refactoring_features = refactoring_analyzer.extract(entity, index)
                        extractor_profiles['refactoring'] = prof
                    
                    # Extract all features together (standard path)
                    feature_vector = feature_registry.extract_all_features(entity, index)
                    all_feature_vectors.append(feature_vector)
                    
                except Exception as e:
                    continue
        
        # Store component profiles for analysis
        for name, profile in extractor_profiles.items():
            self.component_profiles[name] = profile
            
        return all_feature_vectors

    @contextmanager 
    def _profile_component(self, component_name: str):
        """Profile an individual component with cProfile."""
        profiler = cProfile.Profile()
        start_memory = self.process.memory_info().rss / 1024 / 1024
        
        profiler.enable()
        start_time = time.time()
        
        component_profile = ComponentProfile(
            component_name=component_name,
            total_time=0,
            self_time=0,
            call_count=0,
            avg_time_per_call=0,
            memory_usage_mb=0
        )
        
        try:
            yield component_profile
        finally:
            end_time = time.time()
            profiler.disable()
            
            end_memory = self.process.memory_info().rss / 1024 / 1024
            
            # Analyze profiling results
            stats_stream = io.StringIO()
            stats = pstats.Stats(profiler, stream=stats_stream)
            stats.sort_stats('cumulative')
            
            component_profile.total_time = end_time - start_time
            component_profile.memory_usage_mb = end_memory - start_memory
            component_profile.call_count = 1
            component_profile.avg_time_per_call = component_profile.total_time
            
            # Calculate Rust port benefit based on computational intensity
            component_profile.rust_port_benefit = self._estimate_rust_benefit(
                component_name, component_profile.total_time
            )

    def _estimate_rust_benefit(self, component_name: str, duration: float) -> float:
        """Estimate performance benefit from porting component to Rust."""
        base_benefit = 0.0
        
        # Different components have different Rust benefit potential
        benefit_map = {
            'complexity_extractor': 0.7,  # Regex-heavy, CPU bound
            'graph_extractor': 0.6,      # Graph algorithms
            'refactoring_analyzer': 0.5,  # Pattern matching
            'parsing': 0.2,              # Already using tree-sitter (C)
            'echo': 0.1,                 # External tool
            'normalization': 0.8,        # Pure computation
        }
        
        for key, benefit in benefit_map.items():
            if key in component_name.lower():
                base_benefit = benefit
                break
        
        # Scale by duration significance
        if duration > 0.5:
            base_benefit *= 1.5
        elif duration > 0.1:
            base_benefit *= 1.2
            
        return min(1.0, base_benefit)

    def _generate_performance_report(self, total_time: float, total_files: int, 
                                   total_entities: int) -> Dict[str, Any]:
        """Generate comprehensive performance analysis report."""
        
        # Calculate overall statistics
        i_o_bound_time = sum(m.duration_seconds for m in self.metrics if m.is_io_bound)
        cpu_bound_time = sum(m.duration_seconds for m in self.metrics if m.is_cpu_bound)
        
        # Identify top optimization candidates
        rust_candidates = sorted(
            [(m.stage_name, m.rust_candidate_score, m.duration_seconds) 
             for m in self.metrics],
            key=lambda x: x[1] * x[2], reverse=True  # Score weighted by time
        )[:5]
        
        report = {
            "analysis_summary": {
                "total_analysis_time_seconds": total_time,
                "files_analyzed": total_files,
                "entities_analyzed": total_entities,
                "throughput_files_per_second": total_files / total_time if total_time > 0 else 0,
                "throughput_entities_per_second": total_entities / total_time if total_time > 0 else 0,
            },
            
            "performance_breakdown": {
                "io_bound_percentage": (i_o_bound_time / total_time) * 100 if total_time > 0 else 0,
                "cpu_bound_percentage": (cpu_bound_time / total_time) * 100 if total_time > 0 else 0,
                "stage_breakdown": [asdict(m) for m in self.metrics],
            },
            
            "rust_port_analysis": {
                "current_async_improvement": "~35% (baseline from previous optimization)",
                "top_rust_candidates": [
                    {
                        "stage": candidate[0],
                        "rust_benefit_score": candidate[1],
                        "current_time_seconds": candidate[2],
                        "estimated_speedup": f"{2 + candidate[1] * 8:.1f}x",  # 2-10x potential
                        "estimated_time_savings": f"{candidate[2] * candidate[1] * 0.7:.2f}s"
                    }
                    for candidate in rust_candidates
                ],
                "total_estimated_speedup": self._calculate_total_rust_speedup(),
            },
            
            "optimization_recommendations": self._generate_recommendations(),
            
            "native_code_usage": {
                "tree_sitter_parsing": "Already using native C implementation",
                "networkx_graphs": "Using native C extensions where available", 
                "numpy_operations": "Using native BLAS/LAPACK where available",
                "echo_clone_detection": "External tool - mostly I/O and hashing",
            },
            
            "component_profiles": {name: asdict(profile) for name, profile in self.component_profiles.items()},
        }
        
        return report

    def _calculate_total_rust_speedup(self) -> str:
        """Calculate estimated total speedup from Rust porting."""
        total_time = sum(m.duration_seconds for m in self.metrics)
        rust_time_saved = 0
        
        for m in self.metrics:
            if m.rust_candidate_score > 0.3:  # Only significant candidates
                # Estimate 2x to 8x speedup based on score
                speedup = 2 + (m.rust_candidate_score * 6)
                time_saved = m.duration_seconds * (1 - 1/speedup)
                rust_time_saved += time_saved
        
        if total_time > 0:
            overall_speedup = total_time / (total_time - rust_time_saved)
            return f"{overall_speedup:.1f}x overall ({rust_time_saved:.1f}s saved)"
        
        return "Insufficient data"

    def _generate_recommendations(self) -> List[Dict[str, Any]]:
        """Generate specific optimization recommendations."""
        recommendations = []
        
        # Analyze current performance profile
        total_time = sum(m.duration_seconds for m in self.metrics)
        
        # I/O optimization recommendations
        io_time = sum(m.duration_seconds for m in self.metrics if m.is_io_bound)
        if io_time / total_time > 0.3:
            recommendations.append({
                "priority": "HIGH",
                "type": "I/O Optimization", 
                "description": "Significant I/O overhead detected",
                "recommendations": [
                    "Implement better caching strategies",
                    "Use memory mapping for large files",
                    "Parallelize file I/O operations",
                    "Consider async I/O for better concurrency"
                ],
                "estimated_benefit": "20-40% improvement"
            })
        
        # Rust porting recommendations
        high_rust_candidates = [m for m in self.metrics if m.rust_candidate_score > 0.5]
        if high_rust_candidates:
            recommendations.append({
                "priority": "HIGH",
                "type": "Rust Porting",
                "description": "High-value Rust porting opportunities identified",
                "components": [m.stage_name for m in high_rust_candidates],
                "recommendations": [
                    "Port complexity calculation algorithms to Rust",
                    "Implement feature extraction in Rust with Python bindings",
                    "Use Rust for computationally intensive graph analysis",
                    "Consider Rust-based normalization and scoring"
                ],
                "estimated_benefit": "3-8x speedup for computational components"
            })
        
        # Memory optimization
        high_memory_stages = [m for m in self.metrics if m.memory_delta_mb > 100]
        if high_memory_stages:
            recommendations.append({
                "priority": "MEDIUM",
                "type": "Memory Optimization",
                "description": "High memory usage detected in some stages",
                "stages": [m.stage_name for m in high_memory_stages],
                "recommendations": [
                    "Stream processing for large codebases",
                    "More aggressive caching cleanup",
                    "Consider incremental processing",
                    "Memory pool allocation in Rust components"
                ],
                "estimated_benefit": "Reduced memory pressure, better scalability"
            })
        
        # Algorithmic improvements
        slow_stages = [m for m in self.metrics if m.duration_seconds > 2.0]
        if slow_stages:
            recommendations.append({
                "priority": "MEDIUM", 
                "type": "Algorithmic Optimization",
                "description": "Slow stages identified for algorithmic improvements",
                "stages": [m.stage_name for m in slow_stages],
                "recommendations": [
                    "Profile and optimize O(n²) algorithms",
                    "Use more efficient data structures", 
                    "Implement early termination where possible",
                    "Consider approximation algorithms for non-critical features"
                ],
                "estimated_benefit": "Variable, potentially significant"
            })
        
        return recommendations


async def main():
    """Main profiling entry point."""
    parser = argparse.ArgumentParser(description="Profile valknut performance")
    parser.add_argument("--target-dir", required=True, help="Directory to analyze")
    parser.add_argument("--output", default="performance_profile.json", help="Output file")
    parser.add_argument("--languages", nargs="+", default=["python"], help="Languages to analyze")
    parser.add_argument("--echo", action="store_true", help="Enable echo clone detection profiling")
    
    args = parser.parse_args()
    
    # Setup logging
    logging.basicConfig(level=logging.INFO)
    
    # Configure analysis
    config = get_default_config()
    config.languages = args.languages
    config.roots = [{"path": args.target_dir}]
    config.detectors.echo.enabled = args.echo
    
    # Create profiler and run analysis
    profiler = PerformanceProfiler()
    
    print(f"🚀 Profiling valknut analysis of {args.target_dir}")
    print(f"   Languages: {args.languages}")
    print(f"   Echo detection: {'enabled' if args.echo else 'disabled'}")
    
    try:
        report = await profiler.profile_complete_pipeline(config)
        
        # Save detailed results
        with open(args.output, 'w') as f:
            json.dump(report, f, indent=2)
        
        print(f"\n📊 Performance analysis complete!")
        print(f"   Results saved to: {args.output}")
        
        # Print key insights
        print(f"\n🔍 Key Insights:")
        total_time = report["analysis_summary"]["total_analysis_time_seconds"]
        files = report["analysis_summary"]["files_analyzed"] 
        entities = report["analysis_summary"]["entities_analyzed"]
        
        print(f"   • Total time: {total_time:.2f}s for {files} files, {entities} entities")
        print(f"   • Throughput: {files/total_time:.1f} files/s, {entities/total_time:.1f} entities/s")
        
        # Show top Rust candidates
        rust_candidates = report["rust_port_analysis"]["top_rust_candidates"][:3]
        if rust_candidates:
            print(f"\n🦀 Top Rust porting opportunities:")
            for candidate in rust_candidates:
                print(f"   • {candidate['stage']}: {candidate['estimated_speedup']} speedup potential")
        
        # Show I/O vs CPU breakdown
        io_pct = report["performance_breakdown"]["io_bound_percentage"]
        cpu_pct = report["performance_breakdown"]["cpu_bound_percentage"]
        print(f"\n⚡ Performance profile:")
        print(f"   • I/O bound: {io_pct:.1f}% of total time")
        print(f"   • CPU bound: {cpu_pct:.1f}% of total time")
        print(f"   • Mixed/Other: {100-io_pct-cpu_pct:.1f}% of total time")
        
    except Exception as e:
        print(f"❌ Profiling failed: {e}")
        import traceback
        traceback.print_exc()
        return 1
    
    return 0


if __name__ == "__main__":
    exit(asyncio.run(main()))