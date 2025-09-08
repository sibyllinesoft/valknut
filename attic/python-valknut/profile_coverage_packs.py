#!/usr/bin/env python3
"""
Profile coverage pack generation to identify performance bottlenecks.
"""

import cProfile
import pstats
import time
import sys
from pathlib import Path
import asyncio

# Add valknut to path
sys.path.insert(0, "/media/nathan/Seagate Hub/Projects/valknut")

from valknut.core.config import RefactorRankConfig, RootConfig
from valknut.core.pipeline import Pipeline


async def profile_coverage_pack_generation(directory: str):
    """Profile coverage pack generation on a directory."""
    print(f"🔍 Profiling coverage pack generation on: {directory}")
    
    # Configure pipeline with coverage packs enabled
    config = RefactorRankConfig()
    config.roots = [RootConfig(path=directory)]
    config.languages = ["typescript", "javascript", "python"]
    config.impact_packs.enable_coverage_packs = True
    config.impact_packs.max_packs = 20  # Limit for testing
    
    # Enable all detectors for comprehensive analysis
    config.detectors.complexity.enabled = True
    config.detectors.graph.enabled = True
    
    pipeline = Pipeline(config)
    
    # Time the overall process
    start_time = time.time()
    
    try:
        result = await pipeline.analyze()
        end_time = time.time()
        
        print(f"✅ Analysis completed in {end_time - start_time:.2f} seconds")
        print(f"📊 Found {len(result.impact_packs)} impact packs")
        print(f"📁 Analyzed {result.total_files} files")
        print(f"🏷️  Found {len(result.ranked_entities)} entities")
        
        # Show impact pack types
        pack_types = {}
        for pack in result.impact_packs:
            pack_type = pack.kind
            pack_types[pack_type] = pack_types.get(pack_type, 0) + 1
        
        print(f"📦 Impact pack breakdown: {pack_types}")
        
        return result
        
    except Exception as e:
        end_time = time.time()
        print(f"❌ Analysis failed after {end_time - start_time:.2f} seconds: {e}")
        return None


def profile_with_cprofile(directory: str):
    """Run the profiling with cProfile to identify bottlenecks."""
    print(f"🔬 Starting detailed profiling...")
    
    # Create profiler
    profiler = cProfile.Profile()
    
    # Profile the async function
    profiler.enable()
    result = asyncio.run(profile_coverage_pack_generation(directory))
    profiler.disable()
    
    # Save and analyze results
    stats = pstats.Stats(profiler)
    stats.sort_stats('cumulative')
    
    print("\n" + "="*80)
    print("TOP 20 FUNCTIONS BY CUMULATIVE TIME:")
    print("="*80)
    stats.print_stats(20)
    
    print("\n" + "="*80) 
    print("COVERAGE-RELATED FUNCTIONS:")
    print("="*80)
    stats.print_stats('coverage')
    
    print("\n" + "="*80)
    print("IMPACT PACK FUNCTIONS:")
    print("="*80) 
    stats.print_stats('impact_pack')
    
    return result


if __name__ == "__main__":
    # Test on a smaller directory first, then the arbiter directory
    test_directories = [
        "/media/nathan/Seagate Hub/Projects/valknut",  # Small test
        "../arbiter"  # The problematic directory
    ]
    
    for directory in test_directories:
        if Path(directory).exists():
            print(f"\n{'='*80}")
            print(f"PROFILING DIRECTORY: {directory}")
            print(f"{'='*80}")
            
            try:
                result = profile_with_cprofile(directory)
                if result:
                    print(f"✅ Successfully profiled {directory}")
                else:
                    print(f"❌ Failed to profile {directory}")
            except Exception as e:
                print(f"❌ Profiling failed for {directory}: {e}")
                import traceback
                traceback.print_exc()
        else:
            print(f"❌ Directory not found: {directory}")