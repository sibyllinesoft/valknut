#!/usr/bin/env python3
"""
Simple performance analysis of valknut components without full pipeline execution.
Analyzes the codebase structure to estimate performance characteristics.
"""

import json
import time
import ast
import re
from pathlib import Path
from typing import Dict, List, Any
from dataclasses import dataclass, asdict


@dataclass
class ComponentAnalysis:
    """Analysis of a valknut component."""
    name: str
    loc: int
    complexity_score: float
    io_operations: int
    cpu_intensive_patterns: int
    native_dependencies: List[str]
    rust_port_score: float
    estimated_time_percentage: float


class ValknutAnalyzer:
    """Analyzes valknut codebase for performance characteristics."""
    
    def __init__(self, valknut_root: Path):
        self.root = valknut_root
        self.components = {}
        
    def analyze_codebase(self) -> Dict[str, Any]:
        """Analyze the valknut codebase for performance patterns."""
        
        # Core components to analyze
        component_paths = {
            "pipeline": "valknut/core/pipeline.py",
            "complexity_detector": "valknut/detectors/complexity.py",
            "graph_detector": "valknut/detectors/graph.py",
            "echo_bridge": "valknut/detectors/echo_bridge.py", 
            "refactoring_analyzer": "valknut/detectors/refactoring.py",
            "python_adapter": "valknut/lang/python_adapter.py",
            "typescript_adapter": "valknut/lang/typescript_adapter.py",
            "file_discovery": "valknut/io/fsrepo.py",
            "feature_extraction": "valknut/core/featureset.py",
            "bayesian_normalization": "valknut/core/bayesian_normalization.py",
        }
        
        analyses = {}
        
        for component_name, component_path in component_paths.items():
            file_path = self.root / component_path
            if file_path.exists():
                analysis = self._analyze_component(component_name, file_path)
                analyses[component_name] = analysis
                print(f"📊 {component_name}: {analysis.loc} LOC, Rust score: {analysis.rust_port_score:.2f}")
        
        return self._generate_performance_profile(analyses)
    
    def _analyze_component(self, name: str, file_path: Path) -> ComponentAnalysis:
        """Analyze individual component for performance characteristics."""
        try:
            content = file_path.read_text(encoding='utf-8', errors='ignore')
            
            # Basic metrics
            loc = len([line for line in content.split('\n') if line.strip()])
            
            # Parse AST for detailed analysis
            try:
                tree = ast.parse(content)
                complexity = self._calculate_ast_complexity(tree)
                io_ops = self._count_io_operations(content)
                cpu_patterns = self._count_cpu_intensive_patterns(content)
                native_deps = self._find_native_dependencies(content)
                
            except SyntaxError:
                complexity = 0
                io_ops = 0
                cpu_patterns = 0
                native_deps = []
            
            # Calculate Rust porting score
            rust_score = self._calculate_rust_score(name, complexity, io_ops, cpu_patterns, loc)
            
            # Estimate time percentage based on empirical knowledge
            time_pct = self._estimate_time_percentage(name, loc, complexity)
            
            return ComponentAnalysis(
                name=name,
                loc=loc,
                complexity_score=complexity,
                io_operations=io_ops,
                cpu_intensive_patterns=cpu_patterns,
                native_dependencies=native_deps,
                rust_port_score=rust_score,
                estimated_time_percentage=time_pct
            )
            
        except Exception as e:
            print(f"Warning: Could not analyze {name}: {e}")
            return ComponentAnalysis(name, 0, 0, 0, 0, [], 0, 0)
    
    def _calculate_ast_complexity(self, tree: ast.AST) -> float:
        """Calculate complexity score from AST."""
        complexity = 0
        
        for node in ast.walk(tree):
            # Control flow complexity
            if isinstance(node, (ast.If, ast.For, ast.While, ast.Try)):
                complexity += 1
            elif isinstance(node, ast.FunctionDef):
                complexity += 0.5
            elif isinstance(node, ast.ClassDef):
                complexity += 0.3
            # Nested structures add more complexity
            elif isinstance(node, ast.ListComp):
                complexity += 0.5
                
        return complexity
    
    def _count_io_operations(self, content: str) -> int:
        """Count I/O operation patterns."""
        io_patterns = [
            r'\.read\(',
            r'\.write\(',
            r'\.open\(',
            r'pathlib\.',
            r'file\.',
            r'\.exists\(',
            r'\.mkdir\(',
            r'subprocess\.',
            r'asyncio\.run\(',
            r'await\s+\w+\(',
        ]
        
        count = 0
        for pattern in io_patterns:
            count += len(re.findall(pattern, content, re.IGNORECASE))
        return count
    
    def _count_cpu_intensive_patterns(self, content: str) -> int:
        """Count CPU-intensive patterns."""
        cpu_patterns = [
            r'for\s+\w+\s+in\s+.*:',  # Loops
            r'while\s+.*:',
            r're\.',  # Regex operations
            r'\.sort\(',
            r'\.join\(',
            r'\.split\(',
            r'\.replace\(',
            r'json\.',
            r'\.encode\(',
            r'\.decode\(',
            r'range\(',
            r'enumerate\(',
            r'map\(',
            r'filter\(',
            r'reduce\(',
            r'complex.*calculation',
            r'algorithm',
            r'\.calculate',
            r'networkx\.',  # Graph algorithms
        ]
        
        count = 0
        for pattern in cpu_patterns:
            count += len(re.findall(pattern, content, re.IGNORECASE))
        return count
    
    def _find_native_dependencies(self, content: str) -> List[str]:
        """Find native dependencies that are already optimized."""
        native_patterns = {
            'tree_sitter': r'tree_sitter',
            'networkx': r'import networkx|from networkx',
            'numpy': r'import numpy|from numpy',
            'pandas': r'import pandas|from pandas', 
            'scipy': r'import scipy|from scipy',
            'lxml': r'import lxml|from lxml',
            'regex': r'import regex',
            'ujson': r'import ujson',
            'pydantic': r'import pydantic|from pydantic',
        }
        
        found = []
        for name, pattern in native_patterns.items():
            if re.search(pattern, content, re.IGNORECASE):
                found.append(name)
        return found
    
    def _calculate_rust_score(self, name: str, complexity: float, io_ops: int, 
                            cpu_patterns: int, loc: int) -> float:
        """Calculate Rust porting benefit score (0-1)."""
        score = 0.0
        
        # Base score from computational intensity
        if cpu_patterns > 20:
            score += 0.4
        elif cpu_patterns > 10:
            score += 0.2
            
        # Bonus for algorithmic complexity
        if complexity > 50:
            score += 0.3
        elif complexity > 20:
            score += 0.2
            
        # Component-specific bonuses
        component_bonuses = {
            'complexity_detector': 0.8,    # Heavy regex and math
            'bayesian_normalization': 0.9, # Pure computation
            'feature_extraction': 0.7,     # Feature calculations
            'refactoring_analyzer': 0.6,   # Pattern matching
            'graph_detector': 0.7,         # Graph algorithms
            'pipeline': 0.3,               # Orchestration, lots of I/O
            'echo_bridge': 0.1,            # External tool wrapper
            'file_discovery': 0.2,         # I/O heavy
            'python_adapter': 0.4,         # Tree-sitter is already C
            'typescript_adapter': 0.4,     # Tree-sitter is already C
        }
        
        score += component_bonuses.get(name, 0.3)
        
        # Penalty for I/O intensive components
        if io_ops > cpu_patterns:
            score *= 0.5
            
        # Penalty for small components (low impact)
        if loc < 100:
            score *= 0.7
            
        return min(1.0, score)
    
    def _estimate_time_percentage(self, name: str, loc: int, complexity: float) -> float:
        """Estimate what percentage of total analysis time this component takes."""
        # Based on empirical understanding of the pipeline
        time_estimates = {
            'pipeline': 5.0,               # Orchestration overhead
            'file_discovery': 8.0,         # File system operations
            'python_adapter': 25.0,        # Tree-sitter parsing 
            'typescript_adapter': 15.0,    # Tree-sitter parsing
            'complexity_detector': 20.0,   # Regex-heavy analysis
            'graph_detector': 15.0,        # Graph construction
            'refactoring_analyzer': 12.0,  # Pattern matching
            'feature_extraction': 8.0,     # Feature aggregation
            'bayesian_normalization': 3.0, # Normalization math
            'echo_bridge': 2.0,            # If enabled, external tool
        }
        
        base_estimate = time_estimates.get(name, 5.0)
        
        # Adjust based on code size and complexity
        size_multiplier = min(2.0, loc / 1000)
        complexity_multiplier = min(2.0, complexity / 100)
        
        return base_estimate * size_multiplier * complexity_multiplier
    
    def _generate_performance_profile(self, analyses: Dict[str, ComponentAnalysis]) -> Dict[str, Any]:
        """Generate comprehensive performance analysis."""
        
        # Calculate totals
        total_loc = sum(a.loc for a in analyses.values())
        total_time_estimate = sum(a.estimated_time_percentage for a in analyses.values())
        
        # Normalize time percentages
        for analysis in analyses.values():
            analysis.estimated_time_percentage = (analysis.estimated_time_percentage / total_time_estimate) * 100
        
        # Identify optimization opportunities
        rust_candidates = sorted(
            [(name, a.rust_port_score, a.estimated_time_percentage) 
             for name, a in analyses.items()],
            key=lambda x: x[1] * x[2], reverse=True
        )[:5]
        
        # Performance categorization
        io_bound = [name for name, a in analyses.items() if a.io_operations > a.cpu_intensive_patterns]
        cpu_bound = [name for name, a in analyses.items() if a.cpu_intensive_patterns > a.io_operations * 2]
        
        report = {
            "analysis_metadata": {
                "total_components_analyzed": len(analyses),
                "total_lines_of_code": total_loc,
                "analysis_timestamp": time.time(),
            },
            
            "component_breakdown": {name: asdict(analysis) for name, analysis in analyses.items()},
            
            "performance_categorization": {
                "io_bound_components": io_bound,
                "cpu_bound_components": cpu_bound,
                "mixed_components": [name for name in analyses.keys() 
                                   if name not in io_bound and name not in cpu_bound]
            },
            
            "rust_porting_analysis": {
                "methodology": "Static analysis based on code patterns and component characteristics",
                "top_candidates": [
                    {
                        "component": candidate[0],
                        "rust_benefit_score": round(candidate[1], 3),
                        "time_percentage": round(candidate[2], 1),
                        "priority_score": round(candidate[1] * candidate[2], 2),
                        "estimated_speedup_range": f"{2 + candidate[1] * 6:.1f}x to {3 + candidate[1] * 12:.1f}x"
                    }
                    for candidate in rust_candidates if candidate[1] > 0.3
                ],
                "total_addressable_time": sum(c[2] for c in rust_candidates[:3] if c[1] > 0.5),
            },
            
            "native_code_usage": {
                "already_optimized": self._get_native_usage_summary(analyses),
                "optimization_headroom": "Significant opportunities in pure Python computational components"
            },
            
            "recommendations": self._generate_static_recommendations(analyses, rust_candidates)
        }
        
        return report
    
    def _get_native_usage_summary(self, analyses: Dict[str, ComponentAnalysis]) -> Dict[str, List[str]]:
        """Summarize current native code usage."""
        usage = {}
        for name, analysis in analyses.items():
            if analysis.native_dependencies:
                usage[name] = analysis.native_dependencies
        return usage
    
    def _generate_static_recommendations(self, analyses: Dict[str, ComponentAnalysis], 
                                       rust_candidates: List) -> List[Dict[str, Any]]:
        """Generate optimization recommendations based on static analysis."""
        recommendations = []
        
        # High-value Rust candidates
        high_value_rust = [c for c in rust_candidates if c[1] > 0.6 and c[2] > 10]
        if high_value_rust:
            recommendations.append({
                "priority": "HIGH", 
                "type": "Rust Porting",
                "rationale": "Components with high computational load and Rust suitability",
                "components": [c[0] for c in high_value_rust],
                "estimated_benefit": "5-15x speedup for targeted components",
                "implementation_effort": "Medium to High",
                "risk": "Low - can be done incrementally with Python bindings"
            })
        
        # Tree-sitter optimization
        parsing_components = [name for name, a in analyses.items() 
                            if 'adapter' in name and a.estimated_time_percentage > 15]
        if parsing_components:
            recommendations.append({
                "priority": "MEDIUM",
                "type": "Parsing Optimization", 
                "rationale": "Tree-sitter parsing takes significant time but is already optimized C code",
                "components": parsing_components,
                "estimated_benefit": "Limited - already using native tree-sitter",
                "alternatives": [
                    "Better caching of parse trees",
                    "Incremental parsing for large files",
                    "Parallel parsing of multiple files"
                ],
                "implementation_effort": "Medium",
                "risk": "Medium - complex caching invalidation"
            })
        
        # I/O optimization
        io_heavy = [name for name, a in analyses.items() if a.io_operations > 20]
        if io_heavy:
            recommendations.append({
                "priority": "MEDIUM",
                "type": "I/O Optimization",
                "rationale": "I/O intensive components limit overall throughput",
                "components": io_heavy,
                "estimated_benefit": "20-50% improvement through better I/O patterns",
                "strategies": [
                    "Async I/O where not already implemented",
                    "Memory mapping for large files",
                    "Better caching strategies",
                    "Batch file operations"
                ],
                "implementation_effort": "Medium",
                "risk": "Low"
            })
        
        # Algorithmic improvements
        complex_components = [name for name, a in analyses.items() if a.complexity_score > 30]
        if complex_components:
            recommendations.append({
                "priority": "LOW",
                "type": "Algorithmic Optimization", 
                "rationale": "Complex components may have algorithmic improvements available",
                "components": complex_components,
                "estimated_benefit": "Variable - depends on algorithm improvements found",
                "approach": "Profile to identify specific bottlenecks, then optimize",
                "implementation_effort": "High",
                "risk": "Medium - requires careful testing"
            })
        
        return recommendations


def main():
    """Run static analysis of valknut performance characteristics."""
    valknut_root = Path(__file__).parent
    analyzer = ValknutAnalyzer(valknut_root)
    
    print("🔍 Analyzing valknut codebase for performance characteristics...")
    print(f"   Root: {valknut_root}")
    
    report = analyzer.analyze_codebase()
    
    # Save results
    output_file = "static_performance_analysis.json"
    with open(output_file, 'w') as f:
        json.dump(report, f, indent=2)
    
    print(f"\n📊 Analysis complete! Results saved to {output_file}")
    
    # Print key insights
    rust_candidates = report["rust_porting_analysis"]["top_candidates"]
    if rust_candidates:
        print(f"\n🦀 Top Rust porting candidates:")
        for candidate in rust_candidates[:3]:
            print(f"   • {candidate['component']}: {candidate['estimated_speedup_range']} potential speedup")
            print(f"     (Score: {candidate['rust_benefit_score']}, Time: {candidate['time_percentage']}%)")
    
    # Show performance categorization
    perf_cat = report["performance_categorization"]
    print(f"\n⚡ Component categorization:")
    print(f"   • I/O bound: {len(perf_cat['io_bound_components'])} components")
    print(f"   • CPU bound: {len(perf_cat['cpu_bound_components'])} components") 
    print(f"   • Mixed: {len(perf_cat['mixed_components'])} components")
    
    # Show recommendations
    recommendations = report["recommendations"]
    print(f"\n💡 Key recommendations:")
    for rec in recommendations:
        print(f"   • {rec['type']} ({rec['priority']} priority): {rec['estimated_benefit']}")


if __name__ == "__main__":
    main()