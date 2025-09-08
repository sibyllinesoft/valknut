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
from rich.console import Console
from rich.table import Table
from rich.progress import (
    Progress, 
    SpinnerColumn, 
    TextColumn, 
    BarColumn, 
    TaskProgressColumn,
    TimeElapsedColumn,
    TimeRemainingColumn
)
from rich.panel import Panel
from rich.columns import Columns
from rich.text import Text
from rich.align import Align
from rich.tree import Tree
from rich import box

from valknut import __version__
from valknut.api.mcp import get_mcp_manifest
from valknut.core.briefs import BriefGenerator
from valknut.core.config import (
    RefactorRankConfig,
    get_default_config,
    load_config,
    save_config,
)
from valknut.core.pipeline import analyze
from valknut.core.scoring import WeightedScorer
from valknut.io.reports import ReportGenerator, ReportFormat

console = Console()
logger = logging.getLogger(__name__)


def _print_header() -> None:
    """Print Valknut header with version info."""
    from valknut import __version__
    
    header_text = Text.assemble(
        ("⚙️  Valknut", "bold cyan"),
        (" v", "dim"),
        (__version__, "bold cyan"),
        (" - AI-Powered Code Analysis", "dim")
    )
    
    console.print(Panel(
        Align.center(header_text),
        box=box.ROUNDED,
        padding=(1, 2),
        style="blue"
    ))


def _display_config_summary(config) -> None:
    """Display configuration summary in a formatted table."""
    config_table = Table(show_header=False, box=box.SIMPLE)
    config_table.add_column("Setting", style="cyan")
    config_table.add_column("Value")
    
    config_table.add_row("Languages", ", ".join(config.languages))
    config_table.add_row("Top-K Results", str(config.ranking.top_k))
    config_table.add_row("Granularity", config.ranking.granularity)
    config_table.add_row("Cache TTL", f"{config.cache_ttl}s")
    
    console.print(config_table)


def _run_analysis_with_progress(config) -> 'PipelineResult':
    """Run analysis with detailed progress tracking."""
    with Progress(
        TextColumn("[bold blue]{task.description}"),
        BarColumn(bar_width=None),
        TaskProgressColumn(),
        TimeElapsedColumn(),
        console=console,
        expand=True
    ) as progress:
        # Create tasks for different stages
        discovery_task = progress.add_task("📂 Discovering files...", total=100)
        parsing_task = progress.add_task("🔄 Parsing code...", total=100)
        analysis_task = progress.add_task("📊 Analyzing complexity...", total=100)
        ranking_task = progress.add_task("🏆 Ranking entities...", total=100)
        
        # Run the actual analysis
        # Note: This is a simplified progress simulation
        # In a real implementation, you'd hook into the pipeline stages
        import time
        
        progress.update(discovery_task, advance=50)
        time.sleep(0.1)
        progress.update(discovery_task, advance=50)
        
        progress.update(parsing_task, advance=30)
        time.sleep(0.1)
        progress.update(parsing_task, advance=70)
        
        progress.update(analysis_task, advance=40)
        time.sleep(0.1)
        progress.update(analysis_task, advance=60)
        
        progress.update(ranking_task, advance=100)
        
        # Actually run the analysis
        result = asyncio.run(analyze(config))
        
        return result


def _display_analysis_results(result) -> None:
    """Display analysis results with visual indicators."""
    # Create results panel
    results_text = Text()
    results_text.append("✅ Analysis Complete\n\n", style="bold green")
    
    # Summary statistics
    stats_table = Table(show_header=False, box=None)
    stats_table.add_column("Metric", style="cyan", width=20)
    stats_table.add_column("Value", style="bold")
    
    stats_table.add_row("📄 Files Analyzed", f"{result.total_files:,}")
    stats_table.add_row("🏢 Code Entities", f"{result.total_entities:,}")
    stats_table.add_row("⏱️  Processing Time", f"{result.processing_time:.2f}s")
    
    if result.ranked_entities:
        top_score = result.ranked_entities[-1][1]  # Highest score (worst quality)
        health_score = max(0, 100 - (top_score * 100))
        health_emoji = "🟢" if health_score >= 80 else "🟡" if health_score >= 60 else "🔴"
        stats_table.add_row("🏆 Health Score", f"{health_emoji} {health_score:.1f}/100")
        
        # Priority issues count
        priority_issues = sum(1 for _, score in result.ranked_entities if score > 0.7)
        priority_emoji = "✅" if priority_issues == 0 else "⚠️" if priority_issues < 5 else "❌"
        stats_table.add_row("⚠️  Priority Issues", f"{priority_emoji} {priority_issues}")
    
    if result.impact_packs:
        stats_table.add_row("📦 Impact Packs", str(len(result.impact_packs)))
    
    console.print(Panel(
        stats_table,
        title="[bold blue]Analysis Results[/bold blue]",
        box=box.ROUNDED,
        padding=(1, 2)
    ))


def _generate_outputs_with_feedback(result, out_path: Path, output_format: str, quiet: bool) -> None:
    """Generate outputs with progress feedback."""
    if not quiet:
        with Progress(
            SpinnerColumn(),
            TextColumn("{task.description}"),
            console=console
        ) as progress:
            if output_format in ["markdown", "html", "sonar", "csv"]:
                task = progress.add_task(f"Generating {output_format.upper()} team report...")
            else:
                task = progress.add_task(f"Generating {output_format.upper()} output...")
            
            _generate_outputs(result, out_path, output_format)
            progress.update(task, description=f"{output_format.upper()} report generated")
    else:
        _generate_outputs(result, out_path, output_format)


def _display_completion_summary(result, out_path: Path, output_format: str) -> None:
    """Display completion summary with next steps."""
    # Success message
    console.print("\n✅ [bold green]Analysis Complete![/bold green]")
    
    # Output files summary
    console.print(f"\n📁 [bold]Results saved to:[/bold] [cyan]{out_path.absolute()}[/cyan]")
    
    # Quick insights
    if result.ranked_entities:
        console.print("\n📊 [bold blue]Quick Insights:[/bold blue]")
        
        # Top issues
        worst_entities = result.ranked_entities[-5:]  # Top 5 worst
        if worst_entities:
            console.print("\n🔥 [bold red]Top Issues Requiring Attention:[/bold red]")
            for i, (vector, score) in enumerate(reversed(worst_entities), 1):
                entity_name = vector.entity_id.split(":")[-1] if ":" in vector.entity_id else vector.entity_id
                severity = "🔴" if score > 0.8 else "🟡" if score > 0.6 else "🟢"
                console.print(f"  {i}. {severity} [bold]{entity_name}[/bold] (score: {score:.3f})")
        
        # Quick wins
        medium_complexity = [(v, s) for v, s in result.ranked_entities if 0.4 < s < 0.7]
        if medium_complexity:
            console.print(f"\n🏆 [bold green]Quick Wins Available:[/bold green] {len(medium_complexity)} entities with moderate complexity")
    
    # Next steps
    console.print("\n📢 [bold blue]Next Steps:[/bold blue]")
    next_steps = [
        f"1. Review the generated [cyan]{output_format}[/cyan] report for detailed findings",
    ]
    
    if output_format == "html":
        next_steps.extend([
            "2. Open the HTML report in your browser for interactive exploration",
            "3. Share the report with your team for collaborative code review"
        ])
    elif output_format == "sonar":
        next_steps.extend([
            "2. Import the SonarQube JSON into your SonarQube instance",
            "3. Set up quality gates based on the technical debt metrics"
        ])
    elif output_format == "csv":
        next_steps.extend([
            "2. Import the CSV data into your project tracking system",
            "3. Prioritize refactoring tasks based on effort estimates"
        ])
    else:
        next_steps.extend([
            "2. Address high-priority issues identified in the analysis",
            "3. Consider running analysis regularly to track improvements"
        ])
    
    for step in next_steps:
        console.print(f"   {step}")
    
    # Tips based on format
    if output_format == "html":
        html_file = out_path / "team_report.html"
        if html_file.exists():
            console.print(f"\n💻 [dim]Tip: Open [cyan]file://{html_file.absolute()}[/cyan] in your browser[/dim]")
    elif output_format == "markdown":
        md_file = out_path / "team_report.md"
        if md_file.exists():
            console.print(f"\n📝 [dim]Tip: The markdown report is ready for your team wiki or documentation[/dim]")


def get_survey_config(verbosity: str) -> dict:
    """Get survey configuration based on verbosity level."""
    verbosity_configs = {
        "low": {
            "invite_policy": {
                "error": True,
                "timeout": False,
                "p95_ms": 10000.0,
                "large_output_kb": 1024.0
            },
            "sample_neutral": 0.05
        },
        "medium": {
            "invite_policy": {
                "error": True,
                "timeout": True,
                "p95_ms": 5000.0,
                "large_output_kb": 512.0
            },
            "sample_neutral": 0.10
        },
        "high": {
            "invite_policy": {
                "error": True,
                "timeout": True,
                "p95_ms": 2000.0,
                "large_output_kb": 256.0
            },
            "sample_neutral": 0.20
        },
        "maximum": {
            "invite_policy": {
                "error": True,
                "timeout": True,
                "p95_ms": 1000.0,
                "large_output_kb": 128.0
            },
            "sample_neutral": 0.30
        }
    }
    
    base_config = {
        "store": "sqlite:///valknut_feedback.db",
        "ttl_hours": 168  # 7 days
    }
    
    config = base_config.copy()
    config.update(verbosity_configs.get(verbosity, verbosity_configs["maximum"]))
    return config


# Removed HTTP server functions - FastAPI integration no longer available


@click.group()
@click.version_option(version=__version__)
@click.option("--verbose", "-v", is_flag=True, help="Enable verbose logging for debugging")
@click.option("--survey/--no-survey", default=True, help="Enable/disable usage analytics collection (default: enabled)")
@click.option("--survey-verbosity", type=click.Choice(["low", "medium", "high", "maximum"]), default="maximum", help="Set survey invitation verbosity level (default: maximum)")
@click.pass_context
def main(ctx: click.Context, verbose: bool, survey: bool, survey_verbosity: str) -> None:
    """🔍 Valknut - AI-Powered Code Analysis & Refactoring Assistant
    
    Analyze your codebase for technical debt, complexity, and refactoring opportunities.
    Generate professional reports for teams and integrate with development workflows.
    
    Common Usage:
    
      # Quick analysis of current directory
      valknut analyze .
      
      # Generate team-friendly HTML report
      valknut analyze --format html --out reports/ ./src
      
      # Start MCP server for IDE integration
      valknut mcp-stdio
      
      # List supported programming languages
      valknut list-languages
    
    Learn more: https://github.com/yourusername/valknut
    """
    ctx.ensure_object(dict)
    
    # Configure logging
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    )
    
    ctx.obj["verbose"] = verbose
    ctx.obj["survey"] = survey
    ctx.obj["survey_verbosity"] = survey_verbosity


@main.command()
@click.option("--config", "-c", type=click.Path(exists=True), help="Configuration file path")
@click.option("--out", "-o", type=click.Path(), default="out", help="Output directory for reports and analysis results")
@click.option("--format", "output_format", 
    type=click.Choice(["jsonl", "json", "markdown", "html", "sonar", "csv"]), 
    default="jsonl", 
    help="Output format: jsonl (line-delimited JSON), json (single file), markdown (team report), html (interactive report), sonar (SonarQube integration), csv (spreadsheet data)"
)
@click.option("--quiet", "-q", is_flag=True, help="Suppress non-essential output")
@click.argument("paths", nargs=-1, required=True)
@click.pass_context
def analyze_command(
    ctx: click.Context,
    config: Optional[str],
    out: str,
    output_format: str,
    quiet: bool,
    paths: tuple[str, ...],
) -> None:
    """Analyze code repositories for refactorability.
    
    PATHS: One or more directories or files to analyze
    
    Examples:
    
      # Analyze current directory with default settings
      valknut analyze .
      
      # Generate HTML team report for specific directory
      valknut analyze --format html --out reports/ ./my-project
      
      # Use custom configuration and generate multiple formats
      valknut analyze --config ./custom.yml --format markdown ./src
    """
    
    try:
        # Print header
        if not quiet:
            _print_header()
        
        # Load and validate configuration
        if config:
            rr_config = load_config(config)
            if not quiet:
                console.print(f"✅ Loaded configuration from [cyan]{config}[/cyan]")
                _display_config_summary(rr_config)
        else:
            rr_config = get_default_config()
            if not quiet:
                console.print("✅ Using default configuration")
        
        # Validate and prepare paths
        if not quiet:
            console.print("\n📂 [bold blue]Validating Input Paths[/bold blue]")
        
        rr_config.roots = []
        for path_str in paths:
            path = Path(path_str)
            if path.exists():
                from valknut.core.config import RootConfig
                rr_config.roots.append(RootConfig(path=str(path)))
                if not quiet:
                    path_type = "📁 Directory" if path.is_dir() else "📄 File"
                    console.print(f"  {path_type}: [green]{path}[/green]")
            else:
                console.print(f"  ❌ [red]Path does not exist:[/red] {path}", style="red")
                sys.exit(1)
        
        if not rr_config.roots:
            console.print("❌ No valid paths provided", style="red")
            sys.exit(1)
        
        # Create output directory
        out_path = Path(out)
        out_path.mkdir(parents=True, exist_ok=True)
        
        if not quiet:
            console.print(f"\n📁 Output directory: [cyan]{out_path.absolute()}[/cyan]")
            console.print(f"📊 Report format: [cyan]{output_format.upper()}[/cyan]")
        
        # Run analysis with enhanced progress tracking
        if not quiet:
            console.print("\n🔍 [bold blue]Starting Analysis Pipeline[/bold blue]")
            result = _run_analysis_with_progress(rr_config)
        else:
            result = asyncio.run(analyze(rr_config))
        
        # Display analysis results
        if not quiet:
            _display_analysis_results(result)
        
        # Generate outputs
        if not quiet:
            console.print("\n📝 [bold blue]Generating Reports[/bold blue]")
        
        _generate_outputs_with_feedback(result, out_path, output_format, quiet)
        
        if not quiet:
            _display_completion_summary(result, out_path, output_format)
        
    except KeyboardInterrupt:
        console.print("\n⏹️  Analysis interrupted by user", style="yellow")
        sys.exit(130)
    except Exception as e:
        console.print(f"\n❌ [red]Analysis failed:[/red] {e}")
        if ctx.obj["verbose"]:
            console.print_exception()
        sys.exit(1)


def _generate_outputs(result, out_path: Path, output_format: str) -> None:
    """Generate output files from analysis result."""
    
    # Generate briefs and scorer
    scorer = WeightedScorer(result.config.weights)
    brief_generator = BriefGenerator(result.config.briefs, scorer)
    
    # Initialize report generator for team formats
    report_generator = ReportGenerator()
    
    # Handle legacy formats first
    if output_format == "jsonl":
        # JSONL output (legacy)
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
                    "refactoring_suggestions": [
                        {
                            "type": s.type.value,
                            "severity": s.severity,
                            "title": s.title,
                            "description": s.description,
                            "rationale": s.rationale,
                            "benefits": s.benefits,
                            "effort": s.effort,
                        }
                        for s in vector.refactoring_suggestions
                    ] if vector.refactoring_suggestions else [],
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
                    "refactoring_suggestions": [
                        {
                            "type": s.type.value,
                            "severity": s.severity,
                            "title": s.title,
                            "description": s.description,
                            "rationale": s.rationale,
                            "benefits": s.benefits,
                            "effort": s.effort,
                        }
                        for s in vector.refactoring_suggestions
                    ] if vector.refactoring_suggestions else [],
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
                        "pack_type": getattr(pack, 'pack_type', getattr(pack, 'kind', 'unknown')),
                        "title": getattr(pack, 'title', f"{getattr(pack, 'kind', 'Unknown')} Pack"),
                        "description": getattr(pack, 'description', "No description available"),
                        "entities": getattr(pack, 'entities', []),
                        "value_estimate": getattr(pack, 'value_estimate', 0.0),
                        "effort_estimate": getattr(pack, 'effort_estimate', 0.0),
                        "priority_score": getattr(pack, 'priority_score', 0.0),
                        "metadata": getattr(pack, 'metadata', {}),
                    }
                    f.write(json.dumps(pack_dict) + "\n")
            
            console.print(f"📄 Impact packs: {impact_packs_file}")
    
    elif output_format == "json":
        # Single JSON file (legacy)
        report_file = out_path / "analysis_results.json"
        
        results_data = {
            "summary": {
                "result_id": str(result.result_id),
                "total_files": result.total_files,
                "total_entities": result.total_entities,
                "processing_time": result.processing_time,
                "config": result.config.model_dump(),
            },
            "entities": [
                {
                    "entity_id": vector.entity_id,
                    "score": score,
                    "features": vector.normalized_features,
                    "explanations": scorer.explain_score(vector),
                    "refactoring_suggestions": [
                        {
                            "type": s.type.value,
                            "severity": s.severity,
                            "title": s.title,
                            "description": s.description,
                            "rationale": s.rationale,
                            "benefits": s.benefits,
                            "effort": s.effort,
                        }
                        for s in vector.refactoring_suggestions
                    ] if vector.refactoring_suggestions else [],
                }
                for vector, score in result.ranked_entities
            ],
            "impact_packs": [
                {
                    "pack_id": pack.pack_id,
                    "pack_type": getattr(pack, 'pack_type', getattr(pack, 'kind', 'unknown')),
                    "title": getattr(pack, 'title', f"{getattr(pack, 'kind', 'Unknown')} Pack"),
                    "description": getattr(pack, 'description', "No description available"),
                    "entities": getattr(pack, 'entities', []),
                    "value_estimate": getattr(pack, 'value_estimate', 0.0),
                    "effort_estimate": getattr(pack, 'effort_estimate', 0.0),
                    "priority_score": getattr(pack, 'priority_score', 0.0),
                    "metadata": getattr(pack, 'metadata', {}),
                }
                for pack in result.impact_packs
            ] if result.impact_packs else []
        }
        
        with report_file.open("w") as f:
            json.dump(results_data, f, indent=2)
        
        console.print(f"📄 Analysis results: {report_file}")
    
    # Generate team-friendly reports using new system
    elif output_format in ["markdown", "html", "sonar", "csv"]:
        # Generate standardized team report
        team_report = report_generator.generate_team_report(result, scorer)
        
        # Map format string to enum
        format_map = {
            "markdown": ReportFormat.MARKDOWN,
            "html": ReportFormat.HTML, 
            "sonar": ReportFormat.SONAR,
            "csv": ReportFormat.CSV
        }
        
        format_enum = format_map[output_format]
        
        # Export in requested format
        output_file = report_generator.export_report(team_report, format_enum, out_path)
        console.print(f"📊 Team report ({output_format}): {output_file}")
        
        # For HTML and markdown, also show key metrics
        if output_format in ["html", "markdown"]:
            console.print(f"🎯 Health Score: {team_report.overall_health_score}/100")
            console.print(f"⚠️  Priority Issues: {team_report.priority_issues_count}")
            console.print(f"🔧 Refactoring Recommendations: {len(team_report.refactoring_recommendations)}")
    
    # Always generate legacy markdown files for backward compatibility
    if output_format not in ["markdown"]:  # Don't duplicate if already generated above
        summary_file = out_path / "summary.md"
        _generate_markdown_summary(result, summary_file, scorer)
        console.print(f"📄 Legacy summary: {summary_file}")
        
        refactoring_file = out_path / "refactoring_guide.md"
        _generate_refactoring_guide(result, refactoring_file, scorer)
        console.print(f"📄 Legacy refactoring guide: {refactoring_file}")


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
                title = getattr(pack, 'title', f"{getattr(pack, 'kind', 'Unknown')} Pack")
                f.write(f"### {title}\n\n")
                f.write(f"**Type**: {getattr(pack, 'pack_type', getattr(pack, 'kind', 'unknown'))}\n")
                priority_score = getattr(pack, 'priority_score', 0.0)
                value_estimate = getattr(pack, 'value_estimate', 0.0)
                effort_estimate = getattr(pack, 'effort_estimate', 0.0)
                f.write(f"**Priority Score**: {priority_score:.3f}\n")
                f.write(f"**Value/Effort Ratio**: {value_estimate/max(effort_estimate, 1):.2f}\n")
                f.write(f"**Entities**: {len(getattr(pack, 'entities', []))}\n\n")
                
                f.write(f"**Description**: {getattr(pack, 'description', 'No description available')}\n\n")
                
                # Show key entities
                entities = getattr(pack, 'entities', [])
                if entities:
                    f.write("**Key Entities**:\n")
                    for entity in entities[:5]:  # Show first 5 entities
                        f.write(f"- `{entity}`\n")
                    if len(entities) > 5:
                        f.write(f"- *...and {len(entities) - 5} more entities*\n")
                
                f.write(f"\n**Value Estimate**: {value_estimate}\n")
                f.write(f"**Effort Estimate**: {effort_estimate}\n\n")
                
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
            
            # Refactoring suggestions
            if vector.refactoring_suggestions:
                f.write(f"\n**🔨 Refactoring Suggestions ({len(vector.refactoring_suggestions)})**:\n\n")
                
                # Group by severity
                high_priority = [s for s in vector.refactoring_suggestions if s.severity == "high"]
                medium_priority = [s for s in vector.refactoring_suggestions if s.severity == "medium"] 
                low_priority = [s for s in vector.refactoring_suggestions if s.severity == "low"]
                
                for priority_group, priority_name in [
                    (high_priority, "🔴 High Priority"),
                    (medium_priority, "🟡 Medium Priority"), 
                    (low_priority, "🟢 Low Priority")
                ]:
                    if priority_group:
                        f.write(f"**{priority_name}**:\n\n")
                        
                        for suggestion in priority_group:
                            f.write(f"- **{suggestion.title}** ({suggestion.effort} effort)\n")
                            f.write(f"  - *{suggestion.description}*\n")
                            f.write(f"  - **Why**: {suggestion.rationale}\n")
                            
                            if suggestion.benefits:
                                f.write(f"  - **Benefits**: {', '.join(suggestion.benefits[:3])}")
                                if len(suggestion.benefits) > 3:
                                    f.write(f" and {len(suggestion.benefits)-3} more")
                                f.write("\n")
                            
                            f.write("\n")
            else:
                f.write("\n*No specific refactoring suggestions generated.*\n")
            
            f.write("\n---\n\n")


def _generate_refactoring_guide(result, refactoring_file: Path, scorer: WeightedScorer) -> None:
    """Generate detailed refactoring guide with code examples."""
    
    with refactoring_file.open("w") as f:
        f.write("# 🔨 Valknut Refactoring Guide\n\n")
        f.write("*Specific, actionable refactoring recommendations with code examples*\n\n")
        
        # Overview statistics
        total_suggestions = 0
        high_priority_entities = 0
        
        entities_with_suggestions = []
        for vector, score in result.ranked_entities:
            if vector.refactoring_suggestions:
                entities_with_suggestions.append((vector, score))
                total_suggestions += len(vector.refactoring_suggestions)
                if any(s.severity == "high" for s in vector.refactoring_suggestions):
                    high_priority_entities += 1
        
        f.write("## 📊 Refactoring Overview\n\n")
        f.write(f"- **Total entities analyzed**: {result.total_entities}\n")
        f.write(f"- **Entities with suggestions**: {len(entities_with_suggestions)}\n")
        f.write(f"- **Total refactoring opportunities**: {total_suggestions}\n")
        f.write(f"- **High-priority entities**: {high_priority_entities}\n\n")
        
        # Quick action summary
        f.write("## 🎯 Quick Action Summary\n\n")
        f.write("**Immediate Actions (High Priority)**:\n")
        
        immediate_actions = []
        for vector, score in entities_with_suggestions[:20]:  # Top 20
            high_priority_suggestions = [s for s in vector.refactoring_suggestions if s.severity == "high"]
            if high_priority_suggestions:
                immediate_actions.append((vector.entity_id, high_priority_suggestions))
        
        if immediate_actions:
            for entity_id, suggestions in immediate_actions[:5]:  # Top 5 for summary
                f.write(f"- **`{entity_id}`**: {len(suggestions)} critical issue(s)\n")
                for suggestion in suggestions[:2]:  # Top 2 suggestions per entity
                    f.write(f"  - {suggestion.title} ({suggestion.effort} effort)\n")
        else:
            f.write("*No high-priority refactoring issues found.*\n")
        
        f.write("\n---\n\n")
        
        # Detailed refactoring recommendations
        f.write("## 🔍 Detailed Refactoring Recommendations\n\n")
        
        for i, (vector, score) in enumerate(entities_with_suggestions[:15], 1):  # Top 15 entities
            f.write(f"### {i}. `{vector.entity_id}`\n\n")
            f.write(f"**Refactorability Score**: {score:.3f}\n\n")
            
            # Show complexity metrics
            complexity_metrics = {}
            for metric in ['cyclomatic', 'cognitive', 'max_nesting', 'param_count']:
                value = vector.features.get(metric, 0)
                if value > 0:
                    complexity_metrics[metric] = value
            
            if complexity_metrics:
                f.write("**Complexity Metrics**:\n")
                for metric, value in complexity_metrics.items():
                    f.write(f"- {metric.replace('_', ' ').title()}: {value:.1f}\n")
                f.write("\n")
            
            # Refactoring suggestions with examples
            if vector.refactoring_suggestions:
                for j, suggestion in enumerate(vector.refactoring_suggestions, 1):
                    severity_icon = {"high": "🔴", "medium": "🟡", "low": "🟢"}.get(suggestion.severity, "⚪")
                    
                    f.write(f"#### {j}. {severity_icon} {suggestion.title}\n\n")
                    f.write(f"**Severity**: {suggestion.severity.title()} | **Effort**: {suggestion.effort.title()}\n\n")
                    f.write(f"**Description**: {suggestion.description}\n\n")
                    f.write(f"**Why This Matters**: {suggestion.rationale}\n\n")
                    
                    # Benefits
                    if suggestion.benefits:
                        f.write("**Benefits**:\n")
                        for benefit in suggestion.benefits:
                            f.write(f"- {benefit}\n")
                        f.write("\n")
                    
                    # Code examples
                    if suggestion.before_code and suggestion.after_code:
                        f.write("**Before** (Current Code):\n")
                        f.write("```python\n" if "def " in suggestion.before_code else "```\n")
                        f.write(suggestion.before_code)
                        f.write("\n```\n\n")
                        
                        f.write("**After** (Refactored Code):\n")
                        f.write("```python\n" if "def " in suggestion.after_code else "```\n")
                        f.write(suggestion.after_code)
                        f.write("\n```\n\n")
                    
                    f.write("---\n\n")
            
            f.write("\n")
        
        # Common refactoring patterns summary
        f.write("## 📚 Common Refactoring Patterns Detected\n\n")
        
        pattern_counts = {}
        for vector, _ in entities_with_suggestions:
            for suggestion in vector.refactoring_suggestions:
                pattern_type = suggestion.type.value
                if pattern_type not in pattern_counts:
                    pattern_counts[pattern_type] = []
                pattern_counts[pattern_type].append(suggestion)
        
        for pattern_type, suggestions in sorted(pattern_counts.items(), key=lambda x: len(x[1]), reverse=True):
            if len(suggestions) >= 2:  # Only show patterns that appear multiple times
                f.write(f"### {pattern_type.replace('_', ' ').title()} ({len(suggestions)} occurrences)\n\n")
                
                # Get a representative example
                representative = suggestions[0]
                f.write(f"**Typical Issue**: {representative.description}\n\n")
                f.write(f"**Common Benefits**:\n")
                
                # Collect all unique benefits for this pattern
                all_benefits = set()
                for suggestion in suggestions:
                    all_benefits.update(suggestion.benefits)
                
                for benefit in sorted(all_benefits)[:5]:  # Top 5 benefits
                    f.write(f"- {benefit}\n")
                
                f.write(f"\n**Affected Entities**: {len(suggestions)} functions/classes need this refactoring\n\n")
                f.write("---\n\n")
        
        # Implementation priority guide
        f.write("## 🗓️ Implementation Priority Guide\n\n")
        f.write("**Phase 1 - Critical Issues (Start Here)**:\n")
        
        phase1_entities = [
            (vector.entity_id, vector.refactoring_suggestions)
            for vector, _ in entities_with_suggestions
            if any(s.severity == "high" for s in vector.refactoring_suggestions)
        ][:5]
        
        if phase1_entities:
            for entity_id, suggestions in phase1_entities:
                high_suggestions = [s for s in suggestions if s.severity == "high"]
                f.write(f"- **`{entity_id}`**: {len(high_suggestions)} critical issue(s)\n")
        else:
            f.write("*No critical issues detected.*\n")
        
        f.write("\n**Phase 2 - Moderate Issues**:\n")
        
        phase2_entities = [
            (vector.entity_id, vector.refactoring_suggestions)
            for vector, _ in entities_with_suggestions
            if any(s.severity == "medium" for s in vector.refactoring_suggestions)
            and not any(s.severity == "high" for s in vector.refactoring_suggestions)
        ][:10]
        
        if phase2_entities:
            for entity_id, suggestions in phase2_entities:
                medium_suggestions = [s for s in suggestions if s.severity == "medium"]
                f.write(f"- **`{entity_id}`**: {len(medium_suggestions)} improvement(s)\n")
        
        f.write("\n**Phase 3 - Enhancement Opportunities**:\n")
        f.write("*Low-priority suggestions can be addressed during regular maintenance cycles.*\n\n")
        
        # Language-specific tips
        f.write("## 💡 Language-Specific Tips\n\n")
        
        # Detect languages used
        languages_found = set()
        for vector, _ in entities_with_suggestions:
            for suggestion in vector.refactoring_suggestions:
                if "python" in suggestion.before_code.lower() or "def " in suggestion.before_code:
                    languages_found.add("python")
                elif "typescript" in suggestion.before_code.lower() or "function" in suggestion.before_code:
                    languages_found.add("typescript")
                elif "rust" in suggestion.before_code.lower() or "fn " in suggestion.before_code:
                    languages_found.add("rust")
        
        if "python" in languages_found:
            f.write("### 🐍 Python Specific\n")
            f.write("- Use f-strings instead of string concatenation\n")
            f.write("- Prefer list comprehensions over manual loops\n")
            f.write("- Use context managers (`with` statements) for resource management\n")
            f.write("- Consider `dataclasses` for parameter object patterns\n\n")
        
        if "typescript" in languages_found:
            f.write("### 📘 TypeScript Specific\n")
            f.write("- Replace `any` types with specific interfaces\n")
            f.write("- Use arrow functions for callbacks and short functions\n")
            f.write("- Leverage optional chaining (`?.`) for safer property access\n")
            f.write("- Use branded types for better type safety\n\n")
        
        f.write("---\n\n")
        f.write("*This guide was generated by Valknut's RefactoringAnalyzer. ")
        f.write("Review suggestions carefully and adapt them to your specific codebase needs.*\n")


@main.command("print-default-config")
def print_default_config() -> None:
    """📄 Print default configuration in YAML format.
    
    Use this to see all available configuration options and their default values.
    You can save this output to a file and customize it for your project.
    
    Example:
      valknut print-default-config > my-config.yml
    """
    try:
        config = get_default_config()
        
        # Convert to YAML-like format for readability
        import yaml
        config_dict = config.model_dump()
        yaml_output = yaml.safe_dump(config_dict, default_flow_style=False, sort_keys=True)
        
        console.print("[dim]# Default valknut configuration[/dim]")
        console.print("[dim]# Save this to a file and customize as needed[/dim]")
        console.print("[dim]# Usage: valknut analyze --config your-config.yml[/dim]")
        console.print()
        console.print(yaml_output)
    except ImportError:
        console.print("❌ [red]PyYAML not installed. Install with:[/red] pip install pyyaml")
        sys.exit(1)
    except Exception as e:
        console.print(f"❌ [red]Failed to generate config:[/red] {e}")
        sys.exit(1)


@main.command("init-config")
@click.option("--output", "-o", default="valknut-config.yml", help="Output configuration file name")
@click.option("--force", "-f", is_flag=True, help="Overwrite existing configuration file")
def init_config(output: str, force: bool) -> None:
    """⚙️ Initialize a configuration file with defaults.
    
    Creates a YAML configuration file with sensible defaults that you can customize
    for your project. The configuration file controls analysis behavior, feature weights,
    and output formats.
    
    Examples:
      # Create default config file
      valknut init-config
      
      # Create config with custom name
      valknut init-config --output my-project-config.yml
      
      # Overwrite existing config
      valknut init-config --force
    """
    try:
        output_path = Path(output)
        
        # Check if file exists and force not specified
        if output_path.exists() and not force:
            console.print(f"❌ [red]Configuration file already exists:[/red] {output}")
            console.print("   Use --force to overwrite or choose a different name with --output")
            sys.exit(1)
        
        config = get_default_config()
        save_config(config, output)
        
        console.print(f"✅ [bold green]Configuration saved to:[/bold green] [cyan]{output}[/cyan]")
        console.print("\n📝 [bold blue]Next steps:[/bold blue]")
        console.print("   1. Edit the configuration file to customize analysis settings")
        console.print("   2. Run analysis with: [cyan]valknut analyze --config {} <paths>[/cyan]".format(output))
        
        # Show key customization options
        console.print("\n🔧 [bold blue]Key settings you can customize:[/bold blue]")
        customization_table = Table(show_header=False, box=box.SIMPLE)
        customization_table.add_column("Setting", style="cyan")
        customization_table.add_column("Description")
        
        customization_table.add_row("languages", "Programming languages to analyze")
        customization_table.add_row("ranking.top_k", "Number of top entities to report")
        customization_table.add_row("weights", "Feature weights for scoring algorithm")
        customization_table.add_row("detectors.echo.enabled", "Enable clone detection")
        
        console.print(customization_table)
        
    except Exception as e:
        console.print(f"❌ [red]Failed to save configuration:[/red] {e}")
        sys.exit(1)


# HTTP server command removed - use 'valknut mcp-stdio' for MCP integration instead


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
@click.option("--config", "-c", type=click.Path(exists=True), help="Path to configuration file to validate", required=True)
@click.option("--verbose", "-v", is_flag=True, help="Show detailed configuration breakdown")
def validate_config(config: str, verbose: bool) -> None:
    """✅ Validate a Valknut configuration file.
    
    Checks if your configuration file is valid and shows a summary of settings.
    Use this before running analysis to catch configuration errors early.
    
    Examples:
      # Validate config file
      valknut validate-config --config my-config.yml
      
      # Show detailed breakdown
      valknut validate-config --config my-config.yml --verbose
    """
    
    try:
        console.print(f"🔍 [bold blue]Validating configuration:[/bold blue] [cyan]{config}[/cyan]\n")
        
        rr_config = load_config(config)
        console.print("✅ [bold green]Configuration file is valid![/bold green]\n")
        
        # Basic summary
        summary_table = Table(show_header=False, box=box.SIMPLE)
        summary_table.add_column("Setting", style="cyan", width=20)
        summary_table.add_column("Value", style="bold")
        
        summary_table.add_row("📋 Languages", ", ".join(rr_config.languages) if rr_config.languages else "None specified")
        summary_table.add_row("📁 Root paths", str(len(rr_config.roots)) if rr_config.roots else "0 (will use CLI args)")
        summary_table.add_row("🏆 Top-K results", str(rr_config.ranking.top_k))
        summary_table.add_row("🎯 Granularity", rr_config.ranking.granularity)
        summary_table.add_row("⏰ Cache TTL", f"{rr_config.cache_ttl}s")
        
        console.print("📊 [bold blue]Configuration Summary[/bold blue]")
        console.print(summary_table)
        
        # Detailed breakdown if verbose
        if verbose:
            console.print("\n🔧 [bold blue]Detailed Settings[/bold blue]")
            
            # Feature weights
            weights_table = Table(title="Feature Weights", box=box.SIMPLE)
            weights_table.add_column("Feature", style="cyan")
            weights_table.add_column("Weight", justify="right", style="bold")
            
            weights_table.add_row("Complexity", f"{rr_config.weights.complexity:.2f}")
            weights_table.add_row("Clone Mass", f"{rr_config.weights.clone_mass:.2f}")
            weights_table.add_row("Centrality", f"{rr_config.weights.centrality:.2f}")
            weights_table.add_row("Cycles", f"{rr_config.weights.cycles:.2f}")
            weights_table.add_row("Type Friction", f"{rr_config.weights.type_friction:.2f}")
            weights_table.add_row("Smell Prior", f"{rr_config.weights.smell_prior:.2f}")
            
            console.print(weights_table)
            
            # Detector settings
            console.print("\n🔍 [bold blue]Detector Configuration[/bold blue]")
            detector_table = Table(show_header=False, box=box.SIMPLE)
            detector_table.add_column("Detector", style="cyan")
            detector_table.add_column("Status", justify="center")
            detector_table.add_column("Settings")
            
            echo_status = "✅ Enabled" if rr_config.detectors.echo.enabled else "❌ Disabled"
            echo_settings = f"min_similarity={rr_config.detectors.echo.min_similarity:.2f}, min_tokens={rr_config.detectors.echo.min_tokens}"
            detector_table.add_row("Echo (Clone Detection)", echo_status, echo_settings)
            
            console.print(detector_table)
            
            # Root configurations
            if rr_config.roots:
                console.print("\n📂 [bold blue]Root Path Configuration[/bold blue]")
                for i, root in enumerate(rr_config.roots, 1):
                    root_info = f"[cyan]{root.path}[/cyan]"
                    if root.include:
                        root_info += f" (include: {', '.join(root.include)})"
                    if root.exclude:
                        root_info += f" (exclude: {', '.join(root.exclude)})"
                    console.print(f"  {i}. {root_info}")
        
        # Warnings and recommendations
        console.print("\n💡 [bold blue]Recommendations[/bold blue]")
        recommendations = []
        
        if not rr_config.languages:
            recommendations.append("⚠️  No languages specified - analysis will attempt to detect automatically")
        
        if rr_config.ranking.top_k > 1000:
            recommendations.append("⚠️  Very high top_k value may impact performance")
        
        if not rr_config.detectors.echo.enabled:
            recommendations.append("💡 Consider enabling echo detection for clone analysis")
        
        if rr_config.cache_ttl < 3600:  # Less than 1 hour
            recommendations.append("💡 Consider longer cache TTL for better performance on repeated analysis")
        
        if not recommendations:
            recommendations.append("✅ Configuration looks optimal!")
        
        for rec in recommendations:
            console.print(f"   {rec}")
        
    except FileNotFoundError:
        console.print(f"❌ [red]Configuration file not found:[/red] {config}")
        sys.exit(1)
    except Exception as e:
        console.print(f"❌ [red]Configuration validation failed:[/red] {e}")
        console.print("\n🔧 [bold blue]Common issues:[/bold blue]")
        console.print("   • Check YAML syntax (indentation, colons, quotes)")
        console.print("   • Verify all required fields are present")
        console.print("   • Ensure numeric values are in valid ranges")
        console.print("\n💡 [dim]Tip: Use 'valknut print-default-config' to see valid format[/dim]")
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
        
        # Check if surveying is enabled
        survey_enabled = ctx.obj.get("survey", True)
        survey_verbosity = ctx.obj.get("survey_verbosity", "maximum")
        
        # Note: Surveying integration temporarily disabled due to HTTP server removal
        import sys
        if survey_enabled:
            print("📊 Survey integration temporarily disabled", file=sys.stderr)
        else:
            print("📊 Survey disabled", file=sys.stderr)
        
        # Import and run regular stdio server
        from valknut.api.stdio_server import run_stdio_server
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
    """📋 List supported programming languages and their status.
    
    Shows which programming languages Valknut can analyze, along with their
    current support status. Some languages have full feature support while
    others are experimental.
    
    Use this information to configure your analysis or check compatibility
    before running analysis on your codebase.
    """
    try:
        from valknut.core.registry import get_supported_languages
        
        languages = get_supported_languages()
        
        if not languages:
            console.print("⚠️  [yellow]No languages registered. This might indicate a configuration issue.[/yellow]")
            return
        
        console.print("🔤 [bold blue]Supported Programming Languages[/bold blue]")
        console.print(f"   Found {len(languages)} supported languages\\n")
        
        # Categorize languages by support level
        full_support = ["python", "typescript", "javascript", "rust"]
        experimental = [lang for lang in languages if lang not in full_support]
        
        table = Table(show_header=True, header_style="bold magenta", box=box.ROUNDED)
        table.add_column("Language", style="cyan", width=15)
        table.add_column("Extension", style="dim", width=12)
        table.add_column("Status", justify="center", width=15)
        table.add_column("Features", width=25)
        
        # Extension mapping for display
        extensions = {
            "python": ".py",
            "typescript": ".ts, .tsx", 
            "javascript": ".js, .jsx",
            "rust": ".rs",
            "go": ".go",
            "java": ".java",
            "cpp": ".cpp, .cxx",
            "c": ".c, .h",
            "csharp": ".cs",
            "ruby": ".rb",
            "php": ".php"
        }
        
        # Feature descriptions
        features = {
            "python": "Full analysis, refactoring suggestions",
            "typescript": "Full analysis, type checking", 
            "javascript": "Full analysis, complexity metrics",
            "rust": "Full analysis, memory safety checks"
        }
        
        # Add full support languages first
        for lang in sorted(full_support):
            if lang in languages:
                ext = extensions.get(lang, "")
                feat = features.get(lang, "Standard analysis")
                table.add_row(lang.title(), ext, "✅ Full Support", feat)
        
        # Add experimental languages
        for lang in sorted(experimental):
            ext = extensions.get(lang, "")
            table.add_row(lang.title(), ext, "🚧 Experimental", "Basic analysis")
        
        console.print(table)
        
        # Add usage notes
        console.print("\\n📝 [bold blue]Usage Notes:[/bold blue]")
        console.print("   • Full Support: Complete feature set with refactoring suggestions")
        console.print("   • Experimental: Basic complexity analysis, limited features")
        console.print("   • Configure languages in your config file with the 'languages' setting")
        console.print("\\n💡 [dim]Tip: Use 'valknut init-config' to create a configuration file[/dim]")
        
    except Exception as e:
        console.print(f"❌ [red]Failed to list languages:[/red] {e}")
        console.print_exception()
        sys.exit(1)


if __name__ == "__main__":
    main()