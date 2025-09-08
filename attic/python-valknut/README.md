# Python Valknut (Archived)

This directory contains the original Python implementation of Valknut that has been archived as of September 2024.

## What's Here

This archive contains the complete Python implementation including:

- **`valknut/`** - The main Python package with all modules
- **`tests/`** - Complete Python test suite
- **`pyproject.toml`** - Python project configuration (uv/Poetry)
- **`uv.lock`** - Dependency lock file
- **`pytest.ini`** - Test configuration
- **`*.py`** - Various Python scripts and utilities
- **Coverage data** - Test coverage reports and data
- **Virtual environments** - Python development environments

## Why Archived

The project has transitioned to a Rust implementation for:
- Better performance and memory efficiency
- Improved type safety and reliability
- More maintainable codebase
- Better cross-platform compatibility

## Historical Context

This Python implementation served as the foundation and proof-of-concept for Valknut. The core algorithms, CLI interface, and analysis capabilities were first developed and validated in Python before being reimplemented in Rust.

The Python version includes extensive research on code complexity analysis, AI-assisted refactoring patterns, and performance benchmarking frameworks that informed the Rust implementation.

## For Reference

This archive is preserved for:
- Historical reference
- Algorithm validation
- Performance comparison benchmarks
- Migration assistance if needed

The Rust implementation in the project root provides equivalent (and improved) functionality.