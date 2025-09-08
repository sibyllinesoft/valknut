#!/usr/bin/env python3
"""
Demo script showing the enhanced CLI output capabilities of Valknut.

This script demonstrates the improved formatting, progress indicators,
and user-friendly interface of the enhanced CLI.
"""

import time
from rich.console import Console
from rich.table import Table
from rich.panel import Panel
from rich.text import Text
from rich.align import Align
from rich import box
from rich.progress import Progress, BarColumn, TextColumn, TaskProgressColumn, TimeElapsedColumn

console = Console()

def demo_header():
    """Demonstrate the enhanced header."""
    header_text = Text.assemble(
        ("⚙️  Valknut", "bold cyan"),
        (" v", "dim"),
        ("1.0.0", "bold cyan"),
        (" - AI-Powered Code Analysis", "dim")
    )
    
    console.print(Panel(
        Align.center(header_text),
        box=box.ROUNDED,
        padding=(1, 2),
        style="blue"
    ))

def demo_config_summary():
    """Demonstrate configuration summary display."""
    config_table = Table(show_header=False, box=box.SIMPLE)
    config_table.add_column("Setting", style="cyan")
    config_table.add_column("Value")
    
    config_table.add_row("Languages", "python, typescript, javascript")
    config_table.add_row("Top-K Results", "50")
    config_table.add_row("Granularity", "function")
    config_table.add_row("Cache TTL", "3600s")
    
    console.print("\n📂 [bold blue]Validating Input Paths[/bold blue]")
    console.print("  📁 Directory: [green]./src[/green]")
    console.print("  📄 File: [green]./tests/test_main.py[/green]")
    
    console.print("\n✅ Loaded configuration from [cyan]my-config.yml[/cyan]")
    console.print(config_table)
    
    console.print("\n📁 Output directory: [cyan]/absolute/path/to/out[/cyan]")
    console.print("📊 Report format: [cyan]HTML[/cyan]")

def demo_progress_tracking():
    """Demonstrate enhanced progress tracking."""
    console.print("\n🔍 [bold blue]Starting Analysis Pipeline[/bold blue]")
    
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
        
        # Simulate progress
        for i in range(100):
            time.sleep(0.01)
            if i < 25:
                progress.update(discovery_task, advance=4)
            elif i < 50:
                progress.update(parsing_task, advance=4)
            elif i < 75:
                progress.update(analysis_task, advance=4)
            else:
                progress.update(ranking_task, advance=4)

def demo_analysis_results():
    """Demonstrate analysis results display."""
    # Summary statistics
    stats_table = Table(show_header=False, box=None)
    stats_table.add_column("Metric", style="cyan", width=20)
    stats_table.add_column("Value", style="bold")
    
    stats_table.add_row("📄 Files Analyzed", "1,234")
    stats_table.add_row("🏢 Code Entities", "5,678")
    stats_table.add_row("⏱️  Processing Time", "12.34s")
    stats_table.add_row("🏆 Health Score", "🟡 72.5/100")
    stats_table.add_row("⚠️  Priority Issues", "⚠️ 8")
    stats_table.add_row("📦 Impact Packs", "23")
    
    console.print(Panel(
        stats_table,
        title="[bold blue]Analysis Results[/bold blue]",
        box=box.ROUNDED,
        padding=(1, 2)
    ))

def demo_completion_summary():
    """Demonstrate completion summary with insights."""
    console.print("\n✅ [bold green]Analysis Complete![/bold green]")
    console.print("\n📁 [bold]Results saved to:[/bold] [cyan]/absolute/path/to/out[/cyan]")
    
    console.print("\n📊 [bold blue]Quick Insights:[/bold blue]")
    
    console.print("\n🔥 [bold red]Top Issues Requiring Attention:[/bold red]")
    console.print("  1. 🔴 [bold]calculate_complex_metrics[/bold] (score: 0.892)")
    console.print("  2. 🔴 [bold]process_large_dataset[/bold] (score: 0.845)")
    console.print("  3. 🟡 [bold]handle_user_input[/bold] (score: 0.723)")
    
    console.print("\n🏆 [bold green]Quick Wins Available:[/bold green] 23 entities with moderate complexity")
    
    console.print("\n📢 [bold blue]Next Steps:[/bold blue]")
    console.print("   1. Review the generated [cyan]html[/cyan] report for detailed findings")
    console.print("   2. Open the HTML report in your browser for interactive exploration")
    console.print("   3. Share the report with your team for collaborative code review")
    
    console.print("\n💻 [dim]Tip: Open [cyan]file:///absolute/path/to/out/team_report.html[/cyan] in your browser[/dim]")

def demo_language_listing():
    """Demonstrate language listing functionality."""
    console.print("\n🔤 [bold blue]Supported Programming Languages[/bold blue]")
    console.print("   Found 8 supported languages\n")
    
    table = Table(show_header=True, header_style="bold magenta", box=box.ROUNDED)
    table.add_column("Language", style="cyan", width=15)
    table.add_column("Extension", style="dim", width=12)
    table.add_column("Status", justify="center", width=15)
    table.add_column("Features", width=25)
    
    # Full support languages
    table.add_row("Python", ".py", "✅ Full Support", "Full analysis, refactoring suggestions")
    table.add_row("TypeScript", ".ts, .tsx", "✅ Full Support", "Full analysis, type checking")
    table.add_row("JavaScript", ".js, .jsx", "✅ Full Support", "Full analysis, complexity metrics")
    table.add_row("Rust", ".rs", "✅ Full Support", "Full analysis, memory safety checks")
    
    # Experimental languages
    table.add_row("Go", ".go", "🚧 Experimental", "Basic analysis")
    table.add_row("Java", ".java", "🚧 Experimental", "Basic analysis")
    table.add_row("C++", ".cpp, .cxx", "🚧 Experimental", "Basic analysis")
    
    console.print(table)
    
    console.print("\n📝 [bold blue]Usage Notes:[/bold blue]")
    console.print("   • Full Support: Complete feature set with refactoring suggestions")
    console.print("   • Experimental: Basic complexity analysis, limited features")
    console.print("   • Configure languages in your config file with the 'languages' setting")

def main():
    """Run the CLI output demonstration."""
    console.print("[bold green]🚀 Valknut Enhanced CLI Output Demonstration[/bold green]\n")
    
    console.print("[bold blue]1. Enhanced Header & Configuration Display[/bold blue]")
    demo_header()
    demo_config_summary()
    
    console.print("\n\n[bold blue]2. Improved Progress Tracking[/bold blue]")
    demo_progress_tracking()
    
    console.print("\n\n[bold blue]3. Visual Analysis Results[/bold blue]")
    demo_analysis_results()
    
    console.print("\n\n[bold blue]4. Completion Summary with Insights[/bold blue]")
    demo_completion_summary()
    
    console.print("\n\n[bold blue]5. Enhanced Language Listing[/bold blue]")
    demo_language_listing()
    
    console.print("\n\n[bold green]✨ CLI Enhancement Complete![/bold green]")
    console.print("\n[dim]This demonstrates the improved developer experience with:[/dim]")
    console.print("[dim]• Rich formatted output with colors and emojis[/dim]")
    console.print("[dim]• Clear visual hierarchy and progress indicators[/dim]") 
    console.print("[dim]• Actionable insights and next steps[/dim]")
    console.print("[dim]• Professional error handling and help text[/dim]")
    console.print("[dim]• Comprehensive command examples and usage guidance[/dim]")

if __name__ == "__main__":
    main()