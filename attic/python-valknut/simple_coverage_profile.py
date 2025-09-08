#!/usr/bin/env python3
"""
Simple profiling of coverage pack generation using time measurements.
"""

import time
import sys
import asyncio
from pathlib import Path

# Add valknut to path
sys.path.insert(0, "/media/nathan/Seagate Hub/Projects/valknut")

from valknut.core.config import RefactorRankConfig, RootConfig
from valknut.core.impact_packs import ImpactPackBuilder
from valknut.detectors.coverage import CoverageReportParser


class ProfileTimer:
    """Simple context manager for timing operations."""
    
    def __init__(self, operation_name: str):
        self.operation_name = operation_name
        self.start_time = None
        
    def __enter__(self):
        print(f"⏱️  Starting: {self.operation_name}")
        self.start_time = time.time()
        return self
        
    def __exit__(self, exc_type, exc_val, exc_tb):
        duration = time.time() - self.start_time
        print(f"✅ Completed: {self.operation_name} in {duration:.2f}s")


async def profile_coverage_pack_specific():
    """Profile just the coverage pack generation components."""
    
    print("🔬 Profiling Coverage Pack Generation Components")
    print("="*60)
    
    # Test coverage report parsing first
    coverage_dir = Path("/media/nathan/Seagate Hub/Projects/valknut")
    
    with ProfileTimer("Coverage Report Auto-Detection"):
        parser = CoverageReportParser()
        report_paths = parser._find_coverage_reports(coverage_dir)
        print(f"   Found {len(report_paths)} coverage reports")
    
    if report_paths:
        with ProfileTimer("Coverage Report Parsing"):
            report_path = report_paths[0]
            coverage_report = parser.parse_report(report_path)
            if coverage_report:
                print(f"   Parsed {len(coverage_report.files)} files from coverage report")
    
    # Test impact pack builder initialization
    with ProfileTimer("Impact Pack Builder Initialization"):
        builder = ImpactPackBuilder(
            enable_coverage_packs=True,
            coverage_report_path=str(report_paths[0]) if report_paths else None,
            max_packs=10
        )
        print("   Impact pack builder initialized")
    
    return builder


async def profile_arbiter_directory():
    """Profile analysis of the arbiter directory with timing."""
    
    arbiter_path = Path("../arbiter")
    if not arbiter_path.exists():
        print("❌ Arbiter directory not found")
        return
    
    print(f"\n🎯 Profiling Arbiter Directory Analysis")
    print("="*60)
    
    # Configure for arbiter analysis
    config = RefactorRankConfig()
    config.roots = [RootConfig(path=str(arbiter_path))]
    config.languages = ["typescript", "javascript"] 
    config.impact_packs.enable_coverage_packs = True
    config.impact_packs.max_packs = 5  # Limit for profiling
    config.ranking.top_k = 20  # Limit entities for profiling
    
    # Time individual pipeline stages
    from valknut.core.pipeline import Pipeline
    pipeline = Pipeline(config)
    
    total_start = time.time()
    
    try:
        with ProfileTimer("File Discovery"):
            files = await pipeline._discover_files()
            print(f"   Discovered {len(files)} files")
        
        if len(files) > 1000:
            print(f"⚠️  Large codebase detected: {len(files)} files")
            print("   This could cause performance issues in coverage analysis")
        
        with ProfileTimer("Parse and Index (limited)"):
            # Limit to first 100 files for profiling
            limited_files = files[:100] if len(files) > 100 else files
            parse_indices = await pipeline._parse_and_index(limited_files)
            print(f"   Parsed {len(parse_indices)} files")
        
        with ProfileTimer("Feature Extraction (limited)"):
            feature_vectors = await pipeline._extract_features(parse_indices)
            print(f"   Extracted features for {len(feature_vectors)} entities")
        
        with ProfileTimer("Impact Pack Generation (limited)"):
            impact_packs = await pipeline._generate_impact_packs(
                feature_vectors, parse_indices, files[:100]
            )
            print(f"   Generated {len(impact_packs)} impact packs")
            
            # Show types of impact packs generated
            pack_types = {}
            for pack in impact_packs:
                pack_type = pack.kind
                pack_types[pack_type] = pack_types.get(pack_type, 0) + 1
            print(f"   Pack types: {pack_types}")
        
        total_time = time.time() - total_start
        print(f"\n📊 Total profiling time: {total_time:.2f}s (limited to first 100 files)")
        
        return {
            'files_found': len(files),
            'files_processed': len(limited_files),
            'entities': len(feature_vectors),
            'impact_packs': len(impact_packs),
            'total_time': total_time
        }
        
    except Exception as e:
        total_time = time.time() - total_start
        print(f"❌ Analysis failed after {total_time:.2f}s: {e}")
        import traceback
        traceback.print_exc()
        return None


def profile_impact_pack_code():
    """Profile the impact pack code directly to find bottlenecks."""
    
    print("\n🔍 Profiling Impact Pack Builder Code")
    print("="*60)
    
    # Test coverage report auto-discovery
    test_dir = Path("/media/nathan/Seagate Hub/Projects/valknut")
    
    with ProfileTimer("Coverage Report Auto-Discovery"):
        from valknut.core.impact_packs import ImpactPackBuilder
        builder = ImpactPackBuilder(enable_coverage_packs=True)
        
        # Test the auto-discovery performance 
        common_paths = [
            test_dir / "coverage.xml",
            test_dir / "coverage.json", 
            test_dir / ".coverage",
            test_dir / "htmlcov/index.html",
            test_dir / "coverage/lcov.info",
        ]
        
        found_reports = []
        for path in common_paths:
            if path.exists():
                found_reports.append(path)
                print(f"   Found: {path}")
        
        print(f"   Auto-discovery found {len(found_reports)} reports")
    
    return found_reports


async def main():
    """Main profiling function."""
    print("🚀 Coverage Pack Performance Profiling")
    print("="*80)
    
    # Profile individual components
    await profile_coverage_pack_specific()
    
    # Profile impact pack code
    profile_impact_pack_code()
    
    # Profile arbiter directory (limited)
    result = await profile_arbiter_directory()
    
    if result:
        print(f"\n📈 Performance Analysis:")
        print(f"   Files discovered: {result['files_found']}")
        print(f"   Files processed: {result['files_processed']}")  
        print(f"   Entities analyzed: {result['entities']}")
        print(f"   Impact packs generated: {result['impact_packs']}")
        print(f"   Total time: {result['total_time']:.2f}s")
        
        if result['files_found'] > 1000:
            print(f"\n⚠️  PERFORMANCE BOTTLENECK IDENTIFIED:")
            print(f"   Large codebase with {result['files_found']} files")
            print(f"   Recommend optimizations:")
            print(f"   1. File filtering improvements")
            print(f"   2. Parallel processing")
            print(f"   3. Caching mechanisms")
            print(f"   4. Progress reporting for long operations")


if __name__ == "__main__":
    asyncio.run(main())