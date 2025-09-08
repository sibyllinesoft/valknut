#!/usr/bin/env python3
"""
Team Reporting Demo - Shows how to use valknut's new team reporting features.

This example demonstrates:
1. Basic usage of different report formats
2. Integration with CI/CD pipelines
3. Custom report processing
4. Dashboard integration patterns
"""

import asyncio
import json
import csv
import pandas as pd
from pathlib import Path
from datetime import datetime

# Import valknut components
from valknut.core.config import get_default_config, RootConfig
from valknut.core.pipeline import analyze
from valknut.core.scoring import WeightedScorer
from valknut.io.reports import ReportGenerator, ReportFormat


async def generate_all_report_formats(project_path: str, output_dir: str = "demo_reports"):
    """Generate all available report formats for a project."""
    
    print(f"🔍 Analyzing project: {project_path}")
    print(f"📂 Output directory: {output_dir}")
    
    # Setup configuration
    config = get_default_config()
    config.roots = [RootConfig(path=project_path)]
    
    # Create output directory
    out_path = Path(output_dir)
    out_path.mkdir(exist_ok=True)
    
    try:
        # Run analysis
        result = await analyze(config)
        scorer = WeightedScorer(result.config.weights)
        report_generator = ReportGenerator()
        
        print(f"✅ Analysis complete: {result.total_files} files, {result.total_entities} entities")
        
        # Generate team report structure
        team_report = report_generator.generate_team_report(result, scorer)
        
        print(f"🎯 Overall Health Score: {team_report.overall_health_score}/100")
        print(f"⚠️  Priority Issues: {team_report.priority_issues_count}")
        
        # Generate all formats
        formats_to_generate = [
            (ReportFormat.HTML, "Professional HTML report for presentations"),
            (ReportFormat.MARKDOWN, "Structured markdown for team reviews"),
            (ReportFormat.SONAR, "SonarQube integration format"),
            (ReportFormat.CSV, "Data export for dashboards"),
        ]
        
        generated_files = {}
        
        for report_format, description in formats_to_generate:
            print(f"\n📊 Generating {report_format.value} format...")
            try:
                output_file = report_generator.export_report(team_report, report_format, out_path)
                generated_files[report_format.value] = output_file
                print(f"   ✅ {description}")
                print(f"   📄 File: {output_file}")
            except Exception as e:
                print(f"   ❌ Error generating {report_format.value}: {e}")
        
        return generated_files, team_report
        
    except Exception as e:
        print(f"❌ Analysis failed: {e}")
        raise


def demonstrate_csv_analysis(csv_file_path: Path):
    """Show how to analyze the CSV export with pandas."""
    
    print(f"\n📊 CSV Data Analysis Demo")
    print(f"📂 Loading: {csv_file_path}")
    
    try:
        # Load CSV data
        df = pd.read_csv(csv_file_path)
        
        print(f"📈 Dataset: {len(df)} entities analyzed")
        
        # Basic statistics
        print("\n🔢 Basic Statistics:")
        print(f"   • Average Complexity: {df['Complexity Score'].mean():.3f}")
        print(f"   • Max Complexity: {df['Complexity Score'].max():.3f}")
        print(f"   • High Priority Issues: {len(df[df['Severity'].isin(['BLOCKER', 'CRITICAL'])])}")
        
        # Language breakdown
        print("\n🌐 Language Distribution:")
        lang_stats = df.groupby('Language').agg({
            'Complexity Score': ['count', 'mean', 'max'],
            'Effort Estimate (hours)': 'sum'
        }).round(3)
        print(lang_stats)
        
        # Top issues
        print("\n🚨 Top 5 Critical Issues:")
        top_issues = df.nlargest(5, 'Complexity Score')[
            ['Entity Name', 'Language', 'Complexity Score', 'Severity', 'Effort Estimate (hours)']
        ]
        print(top_issues.to_string(index=False))
        
        # Effort estimation
        total_effort = df['Effort Estimate (hours)'].sum()
        print(f"\n⏱️  Total Estimated Effort: {total_effort:.1f} hours ({total_effort/8:.1f} days)")
        
        return df
        
    except Exception as e:
        print(f"❌ CSV analysis failed: {e}")
        return None


def demonstrate_sonar_integration(sonar_file_path: Path):
    """Show how to work with SonarQube format."""
    
    print(f"\n🔧 SonarQube Integration Demo")
    print(f"📂 Loading: {sonar_file_path}")
    
    try:
        with sonar_file_path.open() as f:
            sonar_data = json.load(f)
        
        issues = sonar_data.get('issues', [])
        rules = sonar_data.get('rules', [])
        
        print(f"📊 SonarQube Export: {len(issues)} issues, {len(rules)} rules")
        
        # Issue breakdown by severity
        severity_counts = {}
        total_effort = 0
        
        for issue in issues:
            severity = issue['severity']
            severity_counts[severity] = severity_counts.get(severity, 0) + 1
            total_effort += issue.get('effortMinutes', 0)
        
        print("\n⚠️  Issues by Severity:")
        for severity, count in sorted(severity_counts.items()):
            print(f"   • {severity}: {count}")
        
        print(f"\n⏱️  Total Effort: {total_effort} minutes ({total_effort/60:.1f} hours)")
        
        # Rule breakdown
        print("\n📋 Available Rules:")
        for rule in rules:
            print(f"   • {rule['name']} ({rule['severity']})")
        
        # Example SonarQube scanner command
        print("\n🔧 SonarQube Integration Command:")
        print("sonar-scanner \\")
        print("  -Dsonar.projectKey=my-project \\")
        print("  -Dsonar.sources=src/ \\")
        print(f"  -Dsonar.externalIssuesReportPaths={sonar_file_path}")
        
        return sonar_data
        
    except Exception as e:
        print(f"❌ SonarQube analysis failed: {e}")
        return None


def demonstrate_ci_cd_integration(generated_files: dict, team_report):
    """Show CI/CD integration patterns."""
    
    print(f"\n🚀 CI/CD Integration Patterns")
    
    # Health score evaluation
    health_score = team_report.overall_health_score
    priority_issues = team_report.priority_issues_count
    
    print(f"🎯 Health Score: {health_score}/100")
    
    # Quality gate logic
    if health_score >= 80 and priority_issues == 0:
        gate_status = "PASS ✅"
        exit_code = 0
    elif health_score >= 60 and priority_issues < 5:
        gate_status = "WARNING ⚠️"
        exit_code = 1
    else:
        gate_status = "FAIL ❌"
        exit_code = 2
    
    print(f"🚪 Quality Gate: {gate_status}")
    
    # Generate CI/CD artifacts
    artifacts = {
        "health_score": health_score,
        "priority_issues": priority_issues,
        "gate_status": gate_status.split()[0],
        "exit_code": exit_code,
        "generated_reports": {k: str(v) for k, v in generated_files.items()},
        "timestamp": datetime.now().isoformat(),
    }
    
    # Save CI/CD metadata
    artifacts_file = Path("demo_reports/ci_artifacts.json")
    with artifacts_file.open("w") as f:
        json.dump(artifacts, f, indent=2)
    
    print(f"📄 CI/CD Artifacts: {artifacts_file}")
    
    # Example GitHub Actions output
    print("\n📝 GitHub Actions Integration:")
    print("- name: Quality Gate Check")
    print("  run: |")
    print(f"    echo 'health_score={health_score}' >> $GITHUB_OUTPUT")
    print(f"    echo 'priority_issues={priority_issues}' >> $GITHUB_OUTPUT")
    print(f"    echo 'gate_status={gate_status.split()[0]}' >> $GITHUB_OUTPUT")
    print(f"    exit {exit_code}")
    
    return artifacts


async def main():
    """Run the complete team reporting demonstration."""
    
    print("🎯 Valknut Team Reporting Demo")
    print("=" * 50)
    
    # Use the valknut codebase itself as demo data
    project_path = "."
    
    try:
        # Step 1: Generate all report formats
        print("\n📊 STEP 1: Generating All Report Formats")
        generated_files, team_report = await generate_all_report_formats(project_path)
        
        # Step 2: CSV data analysis
        if 'csv' in generated_files:
            print("\n📈 STEP 2: CSV Data Analysis")
            csv_df = demonstrate_csv_analysis(generated_files['csv'])
        
        # Step 3: SonarQube integration
        if 'sonar' in generated_files:
            print("\n🔧 STEP 3: SonarQube Integration")
            sonar_data = demonstrate_sonar_integration(generated_files['sonar'])
        
        # Step 4: CI/CD integration patterns
        print("\n🚀 STEP 4: CI/CD Integration")
        ci_artifacts = demonstrate_ci_cd_integration(generated_files, team_report)
        
        # Summary
        print("\n" + "=" * 50)
        print("🎉 Demo Complete!")
        print("\n📂 Generated Files:")
        for format_name, file_path in generated_files.items():
            print(f"   • {format_name.upper()}: {file_path}")
        
        print(f"\n🎯 Project Health: {team_report.overall_health_score}/100")
        print(f"⚠️  Issues to Address: {team_report.priority_issues_count}")
        
        print("\n💡 Next Steps:")
        print("   1. Open team_report.html in your browser for interactive viewing")
        print("   2. Share team_report.md in your team chat or wiki")
        print("   3. Import sonar_issues.json into SonarQube")
        print("   4. Load analysis_data.csv into your dashboard")
        
        # Optional: Open HTML report in browser
        html_file = generated_files.get('html')
        if html_file:
            import webbrowser
            try:
                webbrowser.open(f'file://{html_file.absolute()}')
                print(f"\n🌐 Opening HTML report in browser...")
            except:
                print(f"\n🌐 Open this file in your browser: {html_file.absolute()}")
        
    except Exception as e:
        print(f"\n❌ Demo failed: {e}")
        import traceback
        traceback.print_exc()


if __name__ == "__main__":
    # Run the demo
    asyncio.run(main())