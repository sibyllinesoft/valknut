#!/usr/bin/env python3
"""
Runtime benchmark for valknut components using actual test data.
This provides real performance measurements to validate the static analysis.
"""

import time
import sys
import json
import asyncio
from pathlib import Path
from typing import Dict, List, Any, Optional
from dataclasses import dataclass


# Add valknut to path
sys.path.insert(0, str(Path(__file__).parent))

try:
    from valknut.core.config import get_default_config
    from valknut.core.pipeline import Pipeline
    from valknut.detectors.complexity import ComplexityExtractor  
    from valknut.detectors.graph import GraphExtractor
    from valknut.detectors.refactoring import RefactoringAnalyzer
    from valknut.lang.python_adapter import PythonAdapter
    from valknut.io.fsrepo import FileDiscovery
    from valknut.core.featureset import feature_registry
    VALKNUT_AVAILABLE = True
except ImportError as e:
    print(f"Warning: Valknut not fully available: {e}")
    VALKNUT_AVAILABLE = False


@dataclass
class BenchmarkResult:
    """Results from a benchmark run."""
    component: str
    total_time: float
    items_processed: int
    throughput_per_second: float
    memory_delta_mb: float = 0.0
    error_message: Optional[str] = None


class ValknutRuntimeBenchmark:
    """Benchmark actual valknut performance with real code."""
    
    def __init__(self):
        self.results = []
        
    def run_benchmarks(self) -> Dict[str, Any]:
        """Run all available benchmarks."""
        print("🚀 Running valknut runtime benchmarks...")
        
        if not VALKNUT_AVAILABLE:
            return {"error": "Valknut not available for runtime benchmarking"}
        
        # Test with the datasets in the valknut repo
        test_datasets = [
            "datasets/code-smells-python/command-line-shell/before/src",
            "datasets/code-smells-python/point-of-sale/before",
            "datasets/code-smells-python/employee-management-system",
            "valknut"  # Self-analysis
        ]
        
        results = {}
        
        for dataset_path in test_datasets:
            dataset_dir = Path(dataset_path) 
            if dataset_dir.exists():
                print(f"\n📂 Benchmarking dataset: {dataset_path}")
                dataset_results = self._benchmark_dataset(dataset_dir)
                results[dataset_path] = dataset_results
                break  # Just test one dataset for now to keep runtime reasonable
        
        return {
            "benchmark_results": results,
            "summary": self._generate_summary(results),
            "rust_analysis": self._analyze_rust_potential(results)
        }
    
    def _benchmark_dataset(self, dataset_dir: Path) -> Dict[str, Any]:
        """Benchmark valknut pipeline on a specific dataset."""
        
        # Setup
        config = get_default_config()
        config.languages = ["python"]
        config.roots = [{"path": str(dataset_dir)}]
        config.detectors.echo.enabled = False  # Disable to isolate core performance
        
        dataset_results = {
            "dataset_path": str(dataset_dir),
            "component_benchmarks": {},
            "pipeline_benchmark": None
        }
        
        try:
            # Individual component benchmarks
            dataset_results["component_benchmarks"] = self._benchmark_components(dataset_dir)
            
            # Full pipeline benchmark
            dataset_results["pipeline_benchmark"] = self._benchmark_full_pipeline(config)
            
        except Exception as e:
            dataset_results["error"] = str(e)
            print(f"❌ Dataset benchmark failed: {e}")
        
        return dataset_results
    
    def _benchmark_components(self, dataset_dir: Path) -> Dict[str, BenchmarkResult]:
        """Benchmark individual components."""
        results = {}
        
        # Get Python files for testing
        python_files = list(dataset_dir.glob("**/*.py"))
        if not python_files:
            return {"error": "No Python files found"}
        
        print(f"   Found {len(python_files)} Python files")
        
        # Benchmark file discovery
        results["file_discovery"] = self._benchmark_file_discovery(dataset_dir)
        
        # Benchmark parsing 
        results["python_parsing"] = self._benchmark_python_parsing(python_files)
        
        # Benchmark feature extractors
        if results["python_parsing"].error_message is None:
            # Get some entities to test feature extraction
            adapter = PythonAdapter()
            index = adapter.parse_index(python_files[:3])  # Limit for testing
            entities = list(index.entities.values())[:10]  # Limit entities
            
            if entities:
                results["complexity_extraction"] = self._benchmark_complexity_extraction(entities, index)
                results["graph_extraction"] = self._benchmark_graph_extraction(entities, index)
                results["refactoring_analysis"] = self._benchmark_refactoring_analysis(entities, index)
        
        return results
    
    def _benchmark_file_discovery(self, dataset_dir: Path) -> BenchmarkResult:
        """Benchmark file discovery performance."""
        try:
            start_time = time.time()
            
            discovery = FileDiscovery()
            files = discovery.discover(
                roots=[str(dataset_dir)],
                include_patterns=[],
                exclude_patterns=[],
                languages=["python"]
            )
            
            end_time = time.time()
            duration = end_time - start_time
            
            return BenchmarkResult(
                component="file_discovery",
                total_time=duration,
                items_processed=len(files),
                throughput_per_second=len(files) / duration if duration > 0 else 0
            )
            
        except Exception as e:
            return BenchmarkResult(
                component="file_discovery", 
                total_time=0,
                items_processed=0,
                throughput_per_second=0,
                error_message=str(e)
            )
    
    def _benchmark_python_parsing(self, files: List[Path]) -> BenchmarkResult:
        """Benchmark Python parsing with tree-sitter."""
        try:
            start_time = time.time()
            
            adapter = PythonAdapter()
            index = adapter.parse_index(files)
            
            end_time = time.time()
            duration = end_time - start_time
            
            return BenchmarkResult(
                component="python_parsing",
                total_time=duration, 
                items_processed=len(index.entities),
                throughput_per_second=len(index.entities) / duration if duration > 0 else 0
            )
            
        except Exception as e:
            return BenchmarkResult(
                component="python_parsing",
                total_time=0,
                items_processed=0,
                throughput_per_second=0,
                error_message=str(e)
            )
    
    def _benchmark_complexity_extraction(self, entities, index) -> BenchmarkResult:
        """Benchmark complexity feature extraction."""
        try:
            start_time = time.time()
            
            extractor = ComplexityExtractor()
            features_extracted = 0
            
            for entity in entities:
                if extractor.supports_entity(entity):
                    features = extractor.extract(entity, index)
                    features_extracted += 1
            
            end_time = time.time()
            duration = end_time - start_time
            
            return BenchmarkResult(
                component="complexity_extraction",
                total_time=duration,
                items_processed=features_extracted, 
                throughput_per_second=features_extracted / duration if duration > 0 else 0
            )
            
        except Exception as e:
            return BenchmarkResult(
                component="complexity_extraction",
                total_time=0,
                items_processed=0,
                throughput_per_second=0,
                error_message=str(e)
            )
    
    def _benchmark_graph_extraction(self, entities, index) -> BenchmarkResult:
        """Benchmark graph feature extraction."""
        try:
            start_time = time.time()
            
            extractor = GraphExtractor()
            features_extracted = 0
            
            for entity in entities:
                if extractor.supports_entity(entity):
                    features = extractor.extract(entity, index)
                    features_extracted += 1
            
            end_time = time.time()
            duration = end_time - start_time
            
            return BenchmarkResult(
                component="graph_extraction",
                total_time=duration,
                items_processed=features_extracted,
                throughput_per_second=features_extracted / duration if duration > 0 else 0
            )
            
        except Exception as e:
            return BenchmarkResult(
                component="graph_extraction", 
                total_time=0,
                items_processed=0,
                throughput_per_second=0,
                error_message=str(e)
            )
    
    def _benchmark_refactoring_analysis(self, entities, index) -> BenchmarkResult:
        """Benchmark refactoring analysis."""
        try:
            start_time = time.time()
            
            analyzer = RefactoringAnalyzer()
            features_extracted = 0
            
            for entity in entities:
                if analyzer.supports_entity(entity):
                    features = analyzer.extract(entity, index)
                    features_extracted += 1
            
            end_time = time.time()
            duration = end_time - start_time
            
            return BenchmarkResult(
                component="refactoring_analysis",
                total_time=duration,
                items_processed=features_extracted,
                throughput_per_second=features_extracted / duration if duration > 0 else 0
            )
            
        except Exception as e:
            return BenchmarkResult(
                component="refactoring_analysis",
                total_time=0,
                items_processed=0,
                throughput_per_second=0,
                error_message=str(e)
            )
    
    async def _benchmark_full_pipeline(self, config) -> Dict[str, Any]:
        """Benchmark the complete pipeline."""
        try:
            start_time = time.time()
            
            pipeline = Pipeline(config)
            result = await pipeline.analyze()
            
            end_time = time.time()
            duration = end_time - start_time
            
            return {
                "total_time": duration,
                "files_processed": result.total_files,
                "entities_processed": result.total_entities,
                "throughput_files_per_second": result.total_files / duration if duration > 0 else 0,
                "throughput_entities_per_second": result.total_entities / duration if duration > 0 else 0,
                "processing_time_reported": result.processing_time,
                "top_k_entities": len(result.top_k_entities) if result.top_k_entities else 0
            }
            
        except Exception as e:
            return {"error": str(e)}
    
    def _generate_summary(self, results: Dict[str, Any]) -> Dict[str, Any]:
        """Generate summary of benchmark results."""
        if not results:
            return {"error": "No results to summarize"}
        
        # Get the first (and likely only) dataset results
        dataset_results = list(results.values())[0]
        
        if "error" in dataset_results:
            return {"error": dataset_results["error"]}
        
        component_benchmarks = dataset_results.get("component_benchmarks", {})
        pipeline_benchmark = dataset_results.get("pipeline_benchmark", {})
        
        summary = {
            "pipeline_performance": pipeline_benchmark,
            "component_breakdown": {},
            "bottleneck_analysis": [],
            "rust_timing_validation": {}
        }
        
        # Analyze component performance
        total_component_time = 0
        for component_name, result in component_benchmarks.items():
            if isinstance(result, BenchmarkResult) and result.error_message is None:
                summary["component_breakdown"][component_name] = {
                    "time_seconds": result.total_time,
                    "items_processed": result.items_processed,
                    "throughput": result.throughput_per_second
                }
                total_component_time += result.total_time
        
        # Identify bottlenecks
        if component_benchmarks:
            bottlenecks = []
            for component_name, result in component_benchmarks.items():
                if isinstance(result, BenchmarkResult) and result.error_message is None:
                    if result.total_time > 0.1:  # Significant time
                        bottlenecks.append({
                            "component": component_name,
                            "time_seconds": result.total_time,
                            "percentage_of_components": (result.total_time / total_component_time) * 100 if total_component_time > 0 else 0
                        })
            
            summary["bottleneck_analysis"] = sorted(bottlenecks, key=lambda x: x["time_seconds"], reverse=True)
        
        return summary
    
    def _analyze_rust_potential(self, results: Dict[str, Any]) -> Dict[str, Any]:
        """Analyze Rust porting potential based on runtime data."""
        if not results:
            return {"error": "No results for Rust analysis"}
        
        dataset_results = list(results.values())[0]
        component_benchmarks = dataset_results.get("component_benchmarks", {})
        
        rust_analysis = {
            "runtime_validated_candidates": [],
            "performance_impact_estimate": {},
            "comparison_with_static_analysis": {}
        }
        
        # Analyze each component's Rust potential based on runtime performance
        for component_name, result in component_benchmarks.items():
            if isinstance(result, BenchmarkResult) and result.error_message is None:
                
                # Components taking significant time are Rust candidates
                if result.total_time > 0.05:  # 50ms threshold
                    rust_potential = self._calculate_runtime_rust_potential(
                        component_name, result.total_time, result.throughput_per_second
                    )
                    
                    if rust_potential > 0.3:
                        rust_analysis["runtime_validated_candidates"].append({
                            "component": component_name,
                            "current_time": result.total_time,
                            "current_throughput": result.throughput_per_second,
                            "rust_potential_score": rust_potential,
                            "estimated_speedup": f"{2 + rust_potential * 6:.1f}x",
                            "time_savings_estimate": f"{result.total_time * rust_potential * 0.7:.3f}s"
                        })
        
        return rust_analysis
    
    def _calculate_runtime_rust_potential(self, component_name: str, 
                                        time_taken: float, throughput: float) -> float:
        """Calculate Rust potential based on runtime characteristics."""
        score = 0.0
        
        # Base score from time significance
        if time_taken > 1.0:
            score += 0.5
        elif time_taken > 0.2:
            score += 0.3
        elif time_taken > 0.05:
            score += 0.1
        
        # Component-specific analysis
        component_rust_scores = {
            "complexity_extraction": 0.8,     # CPU-intensive calculations
            "refactoring_analysis": 0.9,      # Pattern matching and analysis
            "graph_extraction": 0.7,          # Graph algorithms (but networkx is already C)
            "python_parsing": 0.3,            # Tree-sitter is already C
            "file_discovery": 0.4,            # Mix of I/O and processing
        }
        
        score += component_rust_scores.get(component_name, 0.5)
        
        # Low throughput indicates computational bottleneck (good for Rust)
        if throughput > 0:
            if throughput < 100:  # entities/second
                score += 0.2
            elif throughput < 1000:
                score += 0.1
        
        return min(1.0, score)


def main():
    """Run runtime benchmarks."""
    benchmark = ValknutRuntimeBenchmark()
    
    try:
        results = benchmark.run_benchmarks()
        
        # Save results
        output_file = "runtime_benchmark_results.json"
        with open(output_file, 'w') as f:
            json.dump(results, f, indent=2, default=str)
        
        print(f"\n📊 Runtime benchmark complete! Results saved to {output_file}")
        
        # Print key results
        if "error" not in results:
            summary = results.get("summary", {})
            pipeline_perf = summary.get("pipeline_performance", {})
            
            if pipeline_perf and "error" not in pipeline_perf:
                total_time = pipeline_perf.get("total_time", 0)
                files = pipeline_perf.get("files_processed", 0)
                entities = pipeline_perf.get("entities_processed", 0)
                
                print(f"\n⚡ Pipeline Performance:")
                print(f"   • Total time: {total_time:.2f}s")
                print(f"   • Files processed: {files}")
                print(f"   • Entities processed: {entities}")
                print(f"   • Throughput: {files/total_time:.1f} files/s, {entities/total_time:.1f} entities/s")
            
            # Show component breakdown
            component_breakdown = summary.get("component_breakdown", {})
            if component_breakdown:
                print(f"\n🔧 Component Performance:")
                for component, perf in component_breakdown.items():
                    time_s = perf.get("time_seconds", 0)
                    items = perf.get("items_processed", 0) 
                    throughput = perf.get("throughput", 0)
                    print(f"   • {component}: {time_s:.3f}s for {items} items ({throughput:.1f}/s)")
            
            # Show Rust candidates
            rust_analysis = results.get("rust_analysis", {})
            rust_candidates = rust_analysis.get("runtime_validated_candidates", [])
            if rust_candidates:
                print(f"\n🦀 Runtime-Validated Rust Candidates:")
                for candidate in rust_candidates:
                    comp = candidate["component"]
                    speedup = candidate["estimated_speedup"] 
                    savings = candidate["time_savings_estimate"]
                    print(f"   • {comp}: {speedup} speedup potential ({savings} time savings)")
        
        else:
            print(f"❌ Benchmark failed: {results['error']}")
            
    except Exception as e:
        print(f"❌ Runtime benchmark failed: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    # Use asyncio for pipeline benchmark
    asyncio.run(main())