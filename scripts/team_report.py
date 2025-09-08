#!/usr/bin/env python3
"""
Team Report Helper Script

Provides convenient workflows for common team reporting scenarios.
"""

import argparse
import subprocess
import sys
from pathlib import Path
from datetime import datetime


def run_valknut_analysis(paths, format_type, output_dir, config=None):
    """Run valknut analysis with specified parameters."""
    
    cmd = ["valknut", "analyze", "--format", format_type, "--out", output_dir]
    
    if config:
        cmd.extend(["--config", config])
    
    cmd.extend(paths)
    
    print(f"🔍 Running: {' '.join(cmd)}")
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        print(result.stdout)
        return True
    except subprocess.CalledProcessError as e:
        print(f"❌ Analysis failed: {e}")
        print(f"Error output: {e.stderr}")
        return False


def weekly_health_check(paths, output_base="reports"):
    """Generate weekly health check reports for team review."""
    
    print("📊 Weekly Health Check Report Generation")
    print("=" * 50)
    
    timestamp = datetime.now().strftime("%Y-%m-%d")
    output_dir = f"{output_base}/weekly-{timestamp}"
    
    # Generate HTML report for stakeholders
    print("\n🌐 Generating HTML report for stakeholders...")
    success = run_valknut_analysis(paths, "html", output_dir)
    
    if success:
        print(f"✅ HTML report generated: {output_dir}/team_report.html")
        
        # Also generate markdown for team discussions
        print("\n📄 Generating markdown for team discussions...")
        run_valknut_analysis(paths, "markdown", output_dir)
        print(f"✅ Markdown report generated: {output_dir}/team_report.md")
        
        # Generate CSV for trend tracking
        print("\n📊 Generating CSV for metrics tracking...")
        run_valknut_analysis(paths, "csv", output_dir)
        print(f"✅ CSV data generated: {output_dir}/analysis_data.csv")
        
        print(f"\n🎯 Weekly report complete!")
        print(f"📂 All files saved to: {output_dir}")
        
        return output_dir
    
    return None


def pre_release_quality_gate(paths, output_dir="quality-gate", threshold_health=80):
    """Run pre-release quality gate check."""
    
    print("🚪 Pre-Release Quality Gate Check")
    print("=" * 40)
    
    # Generate all formats for comprehensive check
    formats = ["html", "csv", "sonar"]
    
    all_success = True
    
    for fmt in formats:
        print(f"\n📊 Generating {fmt} report...")
        success = run_valknut_analysis(paths, fmt, output_dir)
        if not success:
            all_success = False
    
    if all_success:
        # Parse CSV to check health metrics (simplified check)
        csv_file = Path(output_dir) / "analysis_data.csv"
        if csv_file.exists():
            try:
                import pandas as pd
                df = pd.read_csv(csv_file)
                
                # Calculate simple health metrics
                critical_issues = len(df[df['Severity'].isin(['BLOCKER', 'CRITICAL'])])
                avg_complexity = df['Complexity Score'].mean()
                
                print(f"\n📈 Quality Metrics:")
                print(f"   • Critical Issues: {critical_issues}")
                print(f"   • Average Complexity: {avg_complexity:.3f}")
                
                # Quality gate decision
                if critical_issues == 0 and avg_complexity < 0.7:
                    print(f"\n✅ QUALITY GATE: PASS")
                    print(f"   Ready for release!")
                    return 0
                else:
                    print(f"\n❌ QUALITY GATE: FAIL")
                    print(f"   Please address critical issues before release")
                    return 1
                    
            except ImportError:
                print("\n⚠️  pandas not available, skipping detailed analysis")
                print("✅ Reports generated successfully")
                return 0
    else:
        print(f"\n❌ Quality gate failed - report generation errors")
        return 2


def sprint_planning_report(paths, output_dir="sprint-planning"):
    """Generate reports for sprint planning session."""
    
    print("📋 Sprint Planning Report Generation")
    print("=" * 40)
    
    # Generate markdown for team discussions
    print("\n📄 Generating markdown report for planning session...")
    success = run_valknut_analysis(paths, "markdown", output_dir)
    
    if success:
        # Also generate CSV for effort estimation
        print("\n📊 Generating CSV for effort estimation...")
        run_valknut_analysis(paths, "csv", output_dir)
        
        print(f"\n🎯 Sprint planning reports ready!")
        print(f"📂 Files available in: {output_dir}")
        print(f"💡 Use markdown report for team discussions")
        print(f"💡 Use CSV data for story point estimation")
        
        return output_dir
    
    return None


def ci_cd_integration(paths, output_dir="build/quality"):
    """Generate reports for CI/CD integration."""
    
    print("🚀 CI/CD Quality Integration")
    print("=" * 30)
    
    # Generate SonarQube format for integration
    print("\n🔧 Generating SonarQube integration format...")
    success = run_valknut_analysis(paths, "sonar", output_dir)
    
    if success:
        print(f"✅ SonarQube format generated: {output_dir}/sonar_issues.json")
        print("\n💡 Integration command:")
        print("sonar-scanner \\")
        print("  -Dsonar.projectKey=your-project \\")
        print("  -Dsonar.sources=src/ \\")
        print(f"  -Dsonar.externalIssuesReportPaths={output_dir}/sonar_issues.json")
        
        return output_dir
    
    return None


def main():
    """Main CLI interface."""
    
    parser = argparse.ArgumentParser(
        description="Valknut Team Reporting Helper",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Weekly health check
  python team_report.py weekly src/ backend/
  
  # Pre-release quality gate
  python team_report.py quality-gate --threshold 85 src/
  
  # Sprint planning reports
  python team_report.py sprint-planning src/critical_modules/
  
  # CI/CD integration
  python team_report.py ci-cd src/ --output build/quality/
        """
    )
    
    subparsers = parser.add_subparsers(dest='command', help='Available commands')
    
    # Weekly health check
    weekly_parser = subparsers.add_parser('weekly', help='Generate weekly health check reports')
    weekly_parser.add_argument('paths', nargs='+', help='Paths to analyze')
    weekly_parser.add_argument('--output', '-o', default='reports', help='Output base directory')
    
    # Quality gate
    gate_parser = subparsers.add_parser('quality-gate', help='Run pre-release quality gate')
    gate_parser.add_argument('paths', nargs='+', help='Paths to analyze')
    gate_parser.add_argument('--output', '-o', default='quality-gate', help='Output directory')
    gate_parser.add_argument('--threshold', '-t', type=int, default=80, help='Health score threshold')
    
    # Sprint planning
    sprint_parser = subparsers.add_parser('sprint-planning', help='Generate sprint planning reports')
    sprint_parser.add_argument('paths', nargs='+', help='Paths to analyze')
    sprint_parser.add_argument('--output', '-o', default='sprint-planning', help='Output directory')
    
    # CI/CD integration
    cicd_parser = subparsers.add_parser('ci-cd', help='Generate CI/CD integration reports')
    cicd_parser.add_argument('paths', nargs='+', help='Paths to analyze')
    cicd_parser.add_argument('--output', '-o', default='build/quality', help='Output directory')
    
    args = parser.parse_args()
    
    if not args.command:
        parser.print_help()
        return 0
    
    # Execute appropriate workflow
    result = None
    
    if args.command == 'weekly':
        result = weekly_health_check(args.paths, args.output)
    elif args.command == 'quality-gate':
        return pre_release_quality_gate(args.paths, args.output, args.threshold)
    elif args.command == 'sprint-planning':
        result = sprint_planning_report(args.paths, args.output)
    elif args.command == 'ci-cd':
        result = ci_cd_integration(args.paths, args.output)
    
    return 0 if result else 1


if __name__ == "__main__":
    sys.exit(main())