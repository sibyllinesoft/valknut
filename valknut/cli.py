"""
Command-line interface for valknut.
"""

import asyncio
import json
import logging
import sys
from pathlib import Path
from typing import Optional

import click
import uvicorn
from rich.console import Console
from rich.table import Table
from rich.progress import Progress, SpinnerColumn, TextColumn

from valknut import __version__
from valknut.api.mcp import get_mcp_manifest
from valknut.api.server import create_app
from valknut.core.briefs import BriefGenerator
from valknut.core.config import (
    RefactorRankConfig,
    get_default_config,
    load_config,
    save_config,
)
from valknut.core.pipeline import analyze
from valknut.core.scoring import WeightedScorer

console = Console()
logger = logging.getLogger(__name__)


@click.group()
@click.version_option(version=__version__)
@click.option("--verbose", "-v", is_flag=True, help="Enable verbose logging")
@click.pass_context
def main(ctx: click.Context, verbose: bool) -> None:
    """Refactor Rank - Static code analysis for refactorability ranking."""
    ctx.ensure_object(dict)
    
    # Configure logging
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    )
    
    ctx.obj["verbose"] = verbose


@main.command()
@click.option("--config", "-c", type=click.Path(exists=True), help="Configuration file")
@click.option("--out", "-o", type=click.Path(), default="out", help="Output directory")
@click.option("--format", "output_format", type=click.Choice(["jsonl", "json", "markdown"]), default="jsonl", help="Output format")
@click.argument("paths", nargs=-1, required=True)
@click.pass_context
def analyze_command(
    ctx: click.Context,
    config: Optional[str],
    out: str,
    output_format: str,
    paths: tuple[str, ...],
) -> None:
    """Analyze code repositories for refactorability."""
    
    try:
        # Load configuration
        if config:
            rr_config = load_config(config)
            console.print(f"✅ Loaded configuration from {config}")
        else:
            rr_config = get_default_config()
            console.print("✅ Using default configuration")
        
        # Update roots with provided paths
        rr_config.roots = []
        for path_str in paths:
            path = Path(path_str)
            if path.exists():
                from valknut.core.config import RootConfig
                rr_config.roots.append(RootConfig(path=str(path)))
                console.print(f"📁 Added path: {path}")
            else:
                console.print(f"❌ Path does not exist: {path}", style="red")
                sys.exit(1)
        
        if not rr_config.roots:
            console.print("❌ No valid paths provided", style="red")
            sys.exit(1)
        
        # Create output directory
        out_path = Path(out)
        out_path.mkdir(parents=True, exist_ok=True)
        
        # Run analysis
        console.print("\n🔍 Starting analysis...")
        
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            console=console,
        ) as progress:
            task = progress.add_task("Analyzing...", total=None)
            
            # Run async analysis
            result = asyncio.run(analyze(rr_config))
            progress.update(task, description="Analysis complete")
        
        console.print(f"\n✅ Analysis completed in {result.processing_time:.2f}s")
        console.print(f"📊 Analyzed {result.total_files} files, {result.total_entities} entities")
        
        # Generate outputs
        _generate_outputs(result, out_path, output_format)
        
        console.print(f"\n📁 Results saved to {out_path}")
        
    except Exception as e:
        console.print(f"\n❌ Analysis failed: {e}", style="red")
        if ctx.obj["verbose"]:
            console.print_exception()
        sys.exit(1)


def _generate_outputs(result, out_path: Path, output_format: str) -> None:
    """Generate output files from analysis result."""
    
    # Generate briefs
    scorer = WeightedScorer(result.config.weights)
    brief_generator = BriefGenerator(result.config.briefs, scorer)
    
    if output_format == "jsonl":
        # JSONL output
        report_file = out_path / "report.jsonl"
        briefs_file = out_path / "topk.briefs.jsonl"
        
        # Write feature report
        with report_file.open("w") as f:
            for vector, score in result.ranked_entities:
                item = {
                    "entity_id": vector.entity_id,
                    "score": score,
                    "features": vector.normalized_features,
                    "explanations": scorer.explain_score(vector),
                }
                f.write(json.dumps(item) + "\n")
        
        console.print(f"📄 Feature report: {report_file}")
        
        # Write briefs (simplified)
        with briefs_file.open("w") as f:
            for vector, score in result.top_k_entities:
                brief_dict = {
                    "entity_id": vector.entity_id,
                    "score": score,
                    "features": vector.normalized_features,
                    "explanations": scorer.explain_score(vector),
                }
                f.write(json.dumps(brief_dict) + "\n")
        
        console.print(f"📄 Top-K briefs: {briefs_file}")
        
        # Write impact packs
        if result.impact_packs:
            impact_packs_file = out_path / "impact_packs.jsonl"
            with impact_packs_file.open("w") as f:
                for pack in result.impact_packs:
                    pack_dict = {
                        "pack_id": pack.pack_id,
                        "pack_type": pack.pack_type,
                        "title": pack.title,
                        "description": pack.description,
                        "entities": pack.entities,
                        "value_estimate": pack.value_estimate,
                        "effort_estimate": pack.effort_estimate,
                        "priority_score": pack.priority_score,
                        "metadata": pack.metadata,
                    }
                    f.write(json.dumps(pack_dict) + "\n")
            
            console.print(f"📄 Impact packs: {impact_packs_file}")
    
    elif output_format == "json":
        # Single JSON file
        report_file = out_path / "analysis_results.json"
        
        results_data = {
            "summary": {
                "result_id": str(result.result_id),
                "total_files": result.total_files,
                "total_entities": result.total_entities,
                "processing_time": result.processing_time,
                "config": result.config.model_dump(exclude={"server"}),
            },
            "entities": [
                {
                    "entity_id": vector.entity_id,
                    "score": score,
                    "features": vector.normalized_features,
                    "explanations": scorer.explain_score(vector),
                }
                for vector, score in result.ranked_entities
            ],
            "impact_packs": [
                {
                    "pack_id": pack.pack_id,
                    "pack_type": pack.pack_type,
                    "title": pack.title,
                    "description": pack.description,
                    "entities": pack.entities,
                    "value_estimate": pack.value_estimate,
                    "effort_estimate": pack.effort_estimate,
                    "priority_score": pack.priority_score,
                    "metadata": pack.metadata,
                }
                for pack in result.impact_packs
            ] if result.impact_packs else []
        }
        
        with report_file.open("w") as f:
            json.dump(results_data, f, indent=2)
        
        console.print(f"📄 Analysis results: {report_file}")
    
    # Generate markdown summary
    summary_file = out_path / "summary.md"
    _generate_markdown_summary(result, summary_file, scorer)
    console.print(f"📄 Summary report: {summary_file}")


def _generate_markdown_summary(result, summary_file: Path, scorer: WeightedScorer) -> None:
    """Generate markdown summary report."""
    
    with summary_file.open("w") as f:
        f.write("# Refactor Rank Analysis Summary\n\n")
        
        # Overview
        f.write("## Overview\n\n")
        f.write(f"- **Files analyzed**: {result.total_files}\n")
        f.write(f"- **Entities analyzed**: {result.total_entities}\n")
        f.write(f"- **Processing time**: {result.processing_time:.2f}s\n")
        f.write(f"- **Top entities**: {len(result.top_k_entities)}\n\n")
        
        # Configuration
        f.write("## Configuration\n\n")
        f.write("### Feature Weights\n\n")
        weights = result.config.weights
        f.write(f"- **Complexity**: {weights.complexity}\n")
        f.write(f"- **Clone Mass**: {weights.clone_mass}\n")
        f.write(f"- **Centrality**: {weights.centrality}\n")
        f.write(f"- **Cycles**: {weights.cycles}\n")
        f.write(f"- **Type Friction**: {weights.type_friction}\n")
        f.write(f"- **Smell Prior**: {weights.smell_prior}\n\n")
        
        # Impact Packs
        if result.impact_packs:
            f.write("## Impact Packs\n\n")
            f.write(f"Generated **{len(result.impact_packs)}** impact packs for coordinated refactoring:\n\n")
            
            for pack in result.impact_packs[:10]:  # Show top 10 packs
                f.write(f"### {pack.title}\n\n")
                f.write(f"**Type**: {pack.pack_type}\n")
                f.write(f"**Priority Score**: {pack.priority_score:.3f}\n")
                f.write(f"**Value/Effort Ratio**: {pack.value_estimate/max(pack.effort_estimate, 1):.2f}\n")
                f.write(f"**Entities**: {len(pack.entities)}\n\n")
                
                f.write(f"**Description**: {pack.description}\n\n")
                
                # Show key entities
                if pack.entities:
                    f.write("**Key Entities**:\n")
                    for entity in pack.entities[:5]:  # Show first 5 entities
                        f.write(f"- `{entity}`\n")
                    if len(pack.entities) > 5:
                        f.write(f"- *...and {len(pack.entities) - 5} more entities*\n")
                
                f.write(f"\n**Value Estimate**: {pack.value_estimate}\n")
                f.write(f"**Effort Estimate**: {pack.effort_estimate}\n\n")
                
                f.write("---\n\n")
        
        # Top entities
        f.write("## Top Refactor Candidates\n\n")
        
        for i, (vector, score) in enumerate(result.top_k_entities[:10], 1):
            f.write(f"### {i}. Entity: `{vector.entity_id}`\n\n")
            f.write(f"**Score**: {score:.3f}\n\n")
            
            # Top features
            top_features = sorted(
                vector.normalized_features.items(),
                key=lambda x: x[1],
                reverse=True
            )[:5]
            
            f.write("**Top Features**:\n")
            for feature_name, feature_value in top_features:
                if feature_value > 0.1:  # Only show significant features
                    f.write(f"- {feature_name}: {feature_value:.3f}\n")
            
            # Explanations
            explanations = scorer.explain_score(vector)
            if explanations:
                f.write("\n**Issues**:\n")
                for explanation in explanations:
                    f.write(f"- {explanation}\n")
            
            f.write("\n---\n\n")


@main.command("print-default-config")
def print_default_config() -> None:
    """Print default configuration to stdout."""
    config = get_default_config()
    
    # Convert to YAML-like format for readability
    import yaml
    config_dict = config.model_dump()
    yaml_output = yaml.safe_dump(config_dict, default_flow_style=False, sort_keys=True)
    
    console.print("# Default valknut configuration")
    console.print(yaml_output)


@main.command("init-config")
@click.option("--output", "-o", default="rr.yml", help="Output configuration file")
def init_config(output: str) -> None:
    """Initialize a configuration file with defaults."""
    config = get_default_config()
    
    try:
        save_config(config, output)
        console.print(f"✅ Configuration saved to {output}")
        console.print("Edit this file to customize your analysis settings.")
    except Exception as e:
        console.print(f"❌ Failed to save configuration: {e}", style="red")
        sys.exit(1)


@main.command()
@click.option("--config", "-c", type=click.Path(exists=True), help="Configuration file")
@click.option("--host", default=None, help="Host to bind to")
@click.option("--port", default=None, type=int, help="Port to bind to")
@click.option("--reload", is_flag=True, help="Enable auto-reload")
@click.pass_context
def serve(
    ctx: click.Context,
    config: Optional[str],
    host: Optional[str],
    port: Optional[int],
    reload: bool,
) -> None:
    """Start the FastAPI server with MCP integration."""
    
    try:
        # Load configuration
        if config:
            rr_config = load_config(config)
            console.print(f"✅ Loaded configuration from {config}")
        else:
            rr_config = get_default_config()
            console.print("✅ Using default configuration")
        
        # Override server settings
        if host:
            rr_config.server.host = host
        if port:
            rr_config.server.port = port
        
        console.print(f"🚀 Starting server on {rr_config.server.host}:{rr_config.server.port}")
        
        # Create and run app
        app = create_app(rr_config)
        
        uvicorn.run(
            app,
            host=rr_config.server.host,
            port=rr_config.server.port,
            reload=reload,
        )
        
    except Exception as e:
        console.print(f"❌ Server failed to start: {e}", style="red")
        if ctx.obj["verbose"]:
            console.print_exception()
        sys.exit(1)


@main.command("mcp-manifest")
@click.option("--output", "-o", help="Output file (default: stdout)")
def mcp_manifest_command(output: Optional[str]) -> None:
    """Generate MCP manifest JSON."""
    
    manifest = get_mcp_manifest()
    manifest_json = json.dumps(manifest.model_dump(), indent=2)
    
    if output:
        with open(output, "w") as f:
            f.write(manifest_json)
        console.print(f"✅ MCP manifest saved to {output}")
    else:
        console.print(manifest_json)


@main.command()
@click.option("--config", "-c", type=click.Path(exists=True), help="Configuration file")
def validate_config(config: str) -> None:
    """Validate a configuration file."""
    
    try:
        rr_config = load_config(config)
        console.print(f"✅ Configuration file {config} is valid")
        
        # Show summary
        console.print("\n📋 Configuration Summary:")
        console.print(f"Languages: {', '.join(rr_config.languages)}")
        console.print(f"Roots: {len(rr_config.roots)} configured")
        console.print(f"Top-K: {rr_config.ranking.top_k}")
        console.print(f"Granularity: {rr_config.ranking.granularity}")
        
    except Exception as e:
        console.print(f"❌ Configuration validation failed: {e}", style="red")
        sys.exit(1)


@main.command("mcp-stdio")
@click.option("--config", "-c", type=click.Path(exists=True), help="Configuration file")
@click.pass_context
def mcp_stdio_command(ctx: click.Context, config: Optional[str]) -> None:
    """Run MCP server over stdio (for Claude Code integration)."""
    
    try:
        # Load configuration
        if config:
            rr_config = load_config(config)
        else:
            rr_config = get_default_config()
        
        # Import and run stdio server
        from valknut.api.stdio_server import run_stdio_server
        
        # Run the stdio server
        asyncio.run(run_stdio_server(rr_config))
        
    except KeyboardInterrupt:
        # Clean exit on Ctrl+C
        pass
    except Exception as e:
        # Log to stderr (won't interfere with stdio)
        import sys
        print(f"MCP stdio server error: {e}", file=sys.stderr)
        sys.exit(1)


@main.command()
def list_languages() -> None:
    """List supported programming languages."""
    from valknut.core.registry import get_supported_languages
    
    languages = get_supported_languages()
    
    console.print("🔤 Supported Languages:")
    
    table = Table(show_header=True, header_style="bold magenta")
    table.add_column("Language", style="cyan")
    table.add_column("Status", justify="center")
    
    for lang in sorted(languages):
        status = "✅ Available" if lang in ["python", "typescript", "javascript", "rust"] else "🚧 Experimental"
        table.add_row(lang, status)
    
    console.print(table)


if __name__ == "__main__":
    main()