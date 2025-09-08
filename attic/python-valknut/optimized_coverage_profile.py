#!/usr/bin/env python3
"""
Optimized coverage pack performance profiling using the actual pipeline.
"""

import time
import sys
import asyncio
import logging
from pathlib import Path

# Add valknut to path
sys.path.insert(0, "/media/nathan/Seagate Hub/Projects/valknut")

from valknut.core.config import RefactorRankConfig, RootConfig
from valknut.core.pipeline import Pipeline

# Set up logging to see optimization details
logging.basicConfig(level=logging.INFO, format='%(levelname)s: %(message)s')

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
        if duration > 10.0:
            print(f"⚠️  Long operation detected: {self.operation_name} took {duration:.2f}s")


async def profile_optimized_coverage_packs():
    """Profile coverage pack generation with optimizations enabled."""
    
    print("🚀 Optimized Coverage Pack Performance Profiling")
    print("="*80)
    
    arbiter_path = Path("../arbiter")
    if not arbiter_path.exists():
        print("❌ Arbiter directory not found")
        return
    
    # Configure with optimizations
    config = RefactorRankConfig()
    
    # Use optimized root config with enhanced exclusions
    root_config = RootConfig(path=str(arbiter_path))
    # The enhanced exclude patterns from config.py will be used automatically
    config.roots = [root_config]
    
    # Focus on the most common languages in arbiter
    config.languages = ["typescript", "javascript", "python"]  # Reduced set
    
    # Enable coverage packs with limits for testing
    config.impact_packs.enable_coverage_packs = True
    config.impact_packs.max_packs = 10  # Limit for performance testing
    
    # Reduce ranking size for performance testing
    config.ranking.top_k = 50  # Reduced from default 100
    
    print(f"📂 Analyzing directory: {arbiter_path}")
    print(f"🎯 Languages: {config.languages}")
    print(f"📦 Max coverage packs: {config.impact_packs.max_packs}")
    print(f"🔝 Top K entities: {config.ranking.top_k}")
    print()
    
    total_start = time.time()
    
    try:
        pipeline = Pipeline(config)
        
        with ProfileTimer("Stage 1: File Discovery (with git optimizations)"):
            files = await pipeline._discover_files()
            print(f"   📁 Discovered {len(files)} files")
            
            if len(files) > 2000:
                print(f"⚠️  Large file count detected: {len(files)} files")
                print("   This may impact performance. Consider more aggressive filtering.")
        
        # Limit files for performance testing
        if len(files) > 1000:
            print(f"🎯 Limiting to first 1000 files for performance testing")
            files = files[:1000]
        
        with ProfileTimer("Stage 2: Parse and Index (limited)"):
            parse_indices = await pipeline._parse_and_index(files)
            total_entities = sum(len(idx.entities) for idx in parse_indices.values())
            print(f"   🏷️  Parsed {len(parse_indices)} language groups")
            print(f"   🎯 Found {total_entities} entities")
        
        with ProfileTimer("Stage 3: Feature Extraction (limited)"):
            feature_vectors = await pipeline._extract_features(parse_indices)
            print(f"   📊 Extracted features for {len(feature_vectors)} entities")
        
        with ProfileTimer("Stage 4: Coverage Pack Generation (limited)"):
            impact_packs = await pipeline._generate_impact_packs(parse_indices)
            
            # Count pack types
            pack_types = {}
            for pack in impact_packs:
                pack_type = pack.kind
                pack_types[pack_type] = pack_types.get(pack_type, 0) + 1
                
            print(f"   📦 Generated {len(impact_packs)} impact packs")
            print(f"   📊 Pack types: {pack_types}")
        
        total_time = time.time() - total_start
        print(f"\n🎯 OPTIMIZATION RESULTS:")
        print(f"   Total time: {total_time:.2f}s")
        print(f"   Files processed: {len(files)}")
        print(f"   Entities analyzed: {len(feature_vectors)}")
        print(f"   Impact packs generated: {len(impact_packs)}")
        print(f"   Average time per file: {total_time/len(files):.3f}s")
        
        # Performance analysis
        if total_time < 60:  # Under 1 minute
            print(f"✅ GOOD PERFORMANCE: Analysis completed in under 1 minute")
        elif total_time < 300:  # Under 5 minutes
            print(f"⚠️  MODERATE PERFORMANCE: Consider further optimizations")
        else:
            print(f"❌ SLOW PERFORMANCE: Significant optimizations needed")
        
        return {
            'total_time': total_time,
            'files_processed': len(files),
            'entities': len(feature_vectors),
            'impact_packs': len(impact_packs),
            'pack_types': pack_types
        }
        
    except Exception as e:
        total_time = time.time() - total_start
        print(f"❌ Analysis failed after {total_time:.2f}s: {e}")
        import traceback
        traceback.print_exc()
        return None


def compare_with_baseline():
    """Compare optimized performance with baseline."""
    print("\n🔬 PERFORMANCE BASELINE COMPARISON:")
    print("   Previous file discovery: 12.19s for 1001 files")
    print("   Previous rate: ~12ms per file")
    print("   Target improvement: >50% faster file discovery")
    print("   Expected git-aware discovery: <2s for 1000+ files")


if __name__ == "__main__":
    try:
        result = asyncio.run(profile_optimized_coverage_packs())
        
        if result:
            compare_with_baseline()
            
            print(f"\n📈 OPTIMIZATION SUMMARY:")
            files = result['files_processed']
            total_time = result['total_time']
            
            # Calculate performance metrics
            files_per_second = files / total_time
            ms_per_file = (total_time / files) * 1000
            
            print(f"   Files per second: {files_per_second:.1f}")
            print(f"   Milliseconds per file: {ms_per_file:.1f}ms")
            
            # Compare with baseline of 12ms per file
            baseline_ms_per_file = 12.0
            if ms_per_file < baseline_ms_per_file:
                improvement = ((baseline_ms_per_file - ms_per_file) / baseline_ms_per_file) * 100
                print(f"✅ IMPROVEMENT: {improvement:.1f}% faster than baseline")
            else:
                regression = ((ms_per_file - baseline_ms_per_file) / baseline_ms_per_file) * 100
                print(f"❌ REGRESSION: {regression:.1f}% slower than baseline")
        
    except Exception as e:
        print(f"❌ Profiling failed: {e}")
        import traceback
        traceback.print_exc()