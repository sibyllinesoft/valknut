# Analysis Pipeline

Orchestrates multi-stage code analysis with parallel execution.

## Stages

- Structure analysis
- Complexity analysis
- Coverage analysis
- LSH clone detection
- Refactoring analysis
- Impact analysis
- Cohesion analysis

## Key Components

- `pipeline_executor.rs` - Main pipeline orchestration
- `pipeline_stages.rs` - Individual stage implementations
- `services.rs` - Stage orchestrator trait and result bundling
- `result_types.rs` - Analysis result types
