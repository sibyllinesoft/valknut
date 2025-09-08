#!/usr/bin/env python3
"""
Direct profiling of specific coverage pack generation bottlenecks.
"""

import time
import sys
from pathlib import Path

# Add valknut to path
sys.path.insert(0, "/media/nathan/Seagate Hub/Projects/valknut")

from valknut.detectors.coverage import CoverageReportParser


def time_function(func, *args, **kwargs):
    """Time a function call and return result + duration."""
    start = time.time()
    result = func(*args, **kwargs)
    duration = time.time() - start
    return result, duration


def profile_coverage_report_parsing():
    """Profile coverage report parsing specifically."""
    print("🔍 Profiling Coverage Report Parsing")
    print("-" * 50)
    
    # Test with our existing coverage.json
    coverage_file = Path("/media/nathan/Seagate Hub/Projects/valknut/coverage.json")
    
    if not coverage_file.exists():
        print("❌ No coverage.json found for testing")
        return None
    
    parser = CoverageReportParser()
    
    # Time format detection
    result, duration = time_function(parser._detect_format, coverage_file)
    print(f"Format detection: {duration:.3f}s -> {result}")
    
    # Time full parsing
    report, duration = time_function(parser.parse, coverage_file)
    print(f"Full parsing: {duration:.3f}s -> {len(report.files) if report else 0} files")
    
    if report:
        # Time uncovered blocks extraction
        blocks, duration = time_function(report.get_uncovered_blocks, 3)
        print(f"Uncovered blocks extraction: {duration:.3f}s -> {len(blocks)} blocks")
        
        # Time worst files calculation
        worst, duration = time_function(report.get_worst_files, 10)
        print(f"Worst files calculation: {duration:.3f}s -> {len(worst)} files")
    
    return report


def profile_impact_pack_builder():
    """Profile the ImpactPackBuilder initialization and auto-discovery."""
    print("\n🏗️  Profiling ImpactPackBuilder")
    print("-" * 50)
    
    from valknut.core.impact_packs import ImpactPackBuilder
    
    # Time builder initialization with coverage enabled
    def create_builder():
        return ImpactPackBuilder(
            enable_coverage_packs=True,
            coverage_report_path=None,  # Force auto-discovery
            max_packs=20
        )
    
    builder, duration = time_function(create_builder)
    print(f"Builder initialization: {duration:.3f}s")
    
    return builder


def profile_file_system_operations():
    """Profile file system operations that might be slow."""
    print("\n📁 Profiling File System Operations") 
    print("-" * 50)
    
    # Test on arbiter directory
    arbiter_path = Path("../arbiter")
    
    if not arbiter_path.exists():
        print("❌ Arbiter directory not found")
        return
    
    # Time directory existence checks
    def check_exists():
        return arbiter_path.exists()
    
    exists, duration = time_function(check_exists)
    print(f"Directory existence check: {duration:.6f}s -> {exists}")
    
    # Time listing directory contents (top level only)
    def list_dir():
        return list(arbiter_path.iterdir())
    
    contents, duration = time_function(list_dir) 
    print(f"Directory listing: {duration:.3f}s -> {len(contents)} items")
    
    # Time recursive file search (limited)
    def find_source_files():
        files = []
        count = 0
        for pattern in ["**/*.ts", "**/*.js", "**/*.py"]:
            for file_path in arbiter_path.glob(pattern):
                files.append(file_path)
                count += 1
                if count > 1000:  # Limit to prevent timeout
                    break
            if count > 1000:
                break
        return files
    
    files, duration = time_function(find_source_files)
    print(f"Source file discovery (limited): {duration:.3f}s -> {len(files)} files")
    
    if duration > 5.0:
        print("⚠️  BOTTLENECK DETECTED: File discovery is very slow!")
        print("   Recommendation: Improve file filtering and use exclude patterns")
    
    return len(files)


def profile_json_loading():
    """Profile JSON loading operations."""
    print("\n📄 Profiling JSON Operations")
    print("-" * 50)
    
    import json
    
    coverage_file = Path("/media/nathan/Seagate Hub/Projects/valknut/coverage.json")
    
    if not coverage_file.exists():
        print("❌ No coverage.json for testing")
        return
    
    # Time JSON loading
    def load_json():
        with open(coverage_file, 'r') as f:
            return json.load(f)
    
    data, duration = time_function(load_json)
    print(f"JSON loading: {duration:.3f}s -> {len(data.get('files', {}))} files")
    
    # Time JSON processing
    def process_files():
        files = data.get('files', {})
        processed = 0
        for file_path, file_data in files.items():
            # Simulate processing each file's coverage data
            missing_lines = file_data.get('missing_lines', [])
            executed_lines = file_data.get('executed_lines', [])
            processed += len(missing_lines) + len(executed_lines)
        return processed
    
    processed, duration = time_function(process_files)
    print(f"JSON processing: {duration:.3f}s -> {processed} lines processed")
    
    return data


def main():
    """Main profiling function."""
    print("🚀 Direct Coverage Pack Performance Profiling")
    print("=" * 80)
    
    # Profile coverage report parsing
    coverage_report = profile_coverage_report_parsing()
    
    # Profile impact pack builder
    impact_builder = profile_impact_pack_builder()
    
    # Profile file system operations (this is likely the bottleneck)
    file_count = profile_file_system_operations()
    
    # Profile JSON operations  
    json_data = profile_json_loading()
    
    print(f"\n📊 Summary:")
    print(f"   Coverage report parsed: {'✅' if coverage_report else '❌'}")
    print(f"   Impact builder created: {'✅' if impact_builder else '❌'}")
    print(f"   Files discovered: {file_count if file_count else 0}")
    print(f"   JSON data loaded: {'✅' if json_data else '❌'}")
    
    if file_count and file_count > 500:
        print(f"\n⚠️  PERFORMANCE ISSUE IDENTIFIED:")
        print(f"   Large codebase with {file_count}+ files")
        print(f"   File discovery is likely the main bottleneck")
        print(f"   Recommendations:")
        print(f"   1. Add better exclude patterns for large directories")
        print(f"   2. Implement file filtering before parsing")
        print(f"   3. Add progress indicators for long operations")
        print(f"   4. Consider parallel file processing")


if __name__ == "__main__":
    main()