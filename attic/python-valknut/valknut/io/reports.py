"""
Professional report generators for team sharing and integration with external tools.
"""

import json
import csv
import re
import html
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Any, Optional, Tuple
from dataclasses import dataclass, field
from enum import Enum

from valknut.core.scoring import WeightedScorer
from valknut.core.featureset import FeatureVector

# Use TYPE_CHECKING to avoid circular import
from typing import TYPE_CHECKING
if TYPE_CHECKING:
    from valknut.core.pipeline import PipelineResult


class ReportFormat(Enum):
    """Supported report output formats."""
    MARKDOWN = "markdown"
    HTML = "html"
    SONAR = "sonar"
    CSV = "csv"


class SeverityLevel(Enum):
    """Severity levels for issues."""
    BLOCKER = "BLOCKER"
    CRITICAL = "CRITICAL"
    MAJOR = "MAJOR"
    MINOR = "MINOR"
    INFO = "INFO"


@dataclass
class TeamReport:
    """Standardized report structure for team consumption."""
    
    # Executive Summary
    project_name: str
    analysis_date: str
    total_files: int
    total_entities: int
    processing_time: float
    overall_health_score: float
    priority_issues_count: int
    
    # Language Breakdown
    language_stats: Dict[str, Dict[str, Any]] = field(default_factory=dict)
    
    # Top Issues
    critical_issues: List[Dict[str, Any]] = field(default_factory=list)
    
    # Refactoring Recommendations
    refactoring_recommendations: List[Dict[str, Any]] = field(default_factory=list)
    
    # Technical Debt Metrics
    debt_metrics: Dict[str, float] = field(default_factory=dict)
    
    # Trends (if available)
    trends: Dict[str, List[float]] = field(default_factory=dict)


class ReportGenerator:
    """Professional report generator with multiple output formats."""
    
    def __init__(self):
        self.timestamp = datetime.now()
        
    def generate_team_report(self, result: 'PipelineResult', scorer: WeightedScorer) -> TeamReport:
        """Generate a standardized team report from pipeline results."""
        
        # Calculate overall health score (0-100)
        health_score = self._calculate_health_score(result, scorer)
        
        # Extract language statistics
        language_stats = self._extract_language_stats(result)
        
        # Identify critical issues
        critical_issues = self._identify_critical_issues(result, scorer)
        
        # Generate refactoring recommendations
        refactoring_recs = self._generate_refactoring_recommendations(result, scorer)
        
        # Calculate technical debt metrics
        debt_metrics = self._calculate_debt_metrics(result, scorer)
        
        return TeamReport(
            project_name=self._extract_project_name(result),
            analysis_date=self.timestamp.isoformat(),
            total_files=result.total_files,
            total_entities=result.total_entities,
            processing_time=result.processing_time,
            overall_health_score=health_score,
            priority_issues_count=len(critical_issues),
            language_stats=language_stats,
            critical_issues=critical_issues,
            refactoring_recommendations=refactoring_recs,
            debt_metrics=debt_metrics
        )
    
    def export_report(self, report: TeamReport, format_type: ReportFormat, output_path: Path) -> Path:
        """Export report in specified format."""
        
        if format_type == ReportFormat.MARKDOWN:
            return self._export_markdown(report, output_path)
        elif format_type == ReportFormat.HTML:
            return self._export_html(report, output_path)
        elif format_type == ReportFormat.SONAR:
            return self._export_sonar(report, output_path)
        elif format_type == ReportFormat.CSV:
            return self._export_csv(report, output_path)
        else:
            raise ValueError(f"Unsupported format: {format_type}")
    
    def _calculate_health_score(self, result: 'PipelineResult', scorer: WeightedScorer) -> float:
        """Calculate overall project health score (0-100)."""
        if not result.ranked_entities:
            return 100.0
        
        # Get top 10 worst entities
        worst_entities = result.ranked_entities[-10:]
        
        # Calculate average score of worst entities
        if worst_entities:
            avg_worst_score = sum(score for _, score in worst_entities) / len(worst_entities)
            # Convert to health score (higher score = worse, so invert)
            health_score = max(0.0, 100.0 - (avg_worst_score * 20))
        else:
            health_score = 100.0
        
        return round(health_score, 1)
    
    def _extract_language_stats(self, result: 'PipelineResult') -> Dict[str, Dict[str, Any]]:
        """Extract statistics per programming language."""
        language_stats = {}
        
        for vector, score in result.ranked_entities:
            # Extract language from entity_id (assuming format like "file.py:function_name")
            parts = vector.entity_id.split(":")
            if len(parts) > 0:
                file_path = parts[0]
                extension = Path(file_path).suffix.lower()
                
                # Map extensions to languages
                lang_map = {
                    ".py": "Python",
                    ".js": "JavaScript", 
                    ".ts": "TypeScript",
                    ".jsx": "JavaScript (JSX)",
                    ".tsx": "TypeScript (JSX)",
                    ".rs": "Rust",
                    ".go": "Go",
                    ".java": "Java",
                    ".cpp": "C++",
                    ".c": "C",
                    ".cs": "C#",
                    ".rb": "Ruby",
                    ".php": "PHP"
                }
                
                language = lang_map.get(extension, "Unknown")
                
                if language not in language_stats:
                    language_stats[language] = {
                        "file_count": 0,
                        "entity_count": 0,
                        "avg_score": 0.0,
                        "max_score": 0.0,
                        "refactoring_suggestions": 0
                    }
                
                stats = language_stats[language]
                stats["entity_count"] += 1
                stats["avg_score"] = (stats["avg_score"] * (stats["entity_count"] - 1) + score) / stats["entity_count"]
                stats["max_score"] = max(stats["max_score"], score)
                stats["refactoring_suggestions"] += len(vector.refactoring_suggestions or [])
                
                # Count unique files
                if file_path not in stats.get("_files", set()):
                    if "_files" not in stats:
                        stats["_files"] = set()
                    stats["_files"].add(file_path)
                    stats["file_count"] += 1
        
        # Clean up temporary file sets and round averages
        for lang_stats in language_stats.values():
            if "_files" in lang_stats:
                del lang_stats["_files"]
            lang_stats["avg_score"] = round(lang_stats["avg_score"], 2)
            lang_stats["max_score"] = round(lang_stats["max_score"], 2)
        
        return language_stats
    
    def _identify_critical_issues(self, result: 'PipelineResult', scorer: WeightedScorer) -> List[Dict[str, Any]]:
        """Identify critical issues requiring immediate attention."""
        critical_issues = []
        
        # Get top 20 worst entities
        worst_entities = result.ranked_entities[-20:] if len(result.ranked_entities) > 20 else result.ranked_entities
        
        for vector, score in reversed(worst_entities):  # Reverse to get highest scores first
            # Only include entities with score > 0.7 or high-severity refactoring suggestions
            has_high_severity = any(
                s.severity == "high" 
                for s in (vector.refactoring_suggestions or [])
            )
            
            if score > 0.7 or has_high_severity:
                issue = {
                    "entity_id": vector.entity_id,
                    "entity_name": self._extract_entity_name(vector.entity_id),
                    "file_path": self._extract_file_path(vector.entity_id),
                    "score": round(score, 3),
                    "severity": self._map_to_severity(score, has_high_severity),
                    "primary_issues": self._extract_primary_issues(vector, scorer),
                    "refactoring_count": len(vector.refactoring_suggestions or []),
                    "high_priority_suggestions": [
                        {
                            "type": s.type.value,
                            "title": s.title,
                            "effort": s.effort
                        }
                        for s in (vector.refactoring_suggestions or [])
                        if s.severity == "high"
                    ]
                }
                critical_issues.append(issue)
        
        return critical_issues
    
    def _generate_refactoring_recommendations(self, result: 'PipelineResult', scorer: WeightedScorer) -> List[Dict[str, Any]]:
        """Generate prioritized refactoring recommendations."""
        recommendations = []
        
        # Group suggestions by type and effort
        suggestion_groups = {}
        
        for vector, score in result.ranked_entities:
            if not vector.refactoring_suggestions:
                continue
                
            for suggestion in vector.refactoring_suggestions:
                key = f"{suggestion.type.value}_{suggestion.effort}"
                
                if key not in suggestion_groups:
                    suggestion_groups[key] = {
                        "type": suggestion.type.value,
                        "effort": suggestion.effort,
                        "count": 0,
                        "avg_score": 0.0,
                        "examples": [],
                        "total_benefit_score": 0.0
                    }
                
                group = suggestion_groups[key]
                group["count"] += 1
                group["avg_score"] = (group["avg_score"] * (group["count"] - 1) + score) / group["count"]
                group["total_benefit_score"] += score * len(suggestion.benefits or [])
                
                if len(group["examples"]) < 3:  # Keep up to 3 examples
                    group["examples"].append({
                        "entity_id": vector.entity_id,
                        "title": suggestion.title,
                        "description": suggestion.description
                    })
        
        # Convert to sorted list
        for group in suggestion_groups.values():
            group["avg_score"] = round(group["avg_score"], 3)
            group["priority_score"] = group["count"] * group["avg_score"] * (1 + group["total_benefit_score"] / 100)
            recommendations.append(group)
        
        # Sort by priority score (descending)
        recommendations.sort(key=lambda x: x["priority_score"], reverse=True)
        
        return recommendations[:15]  # Top 15 recommendations
    
    def _calculate_debt_metrics(self, result: 'PipelineResult', scorer: WeightedScorer) -> Dict[str, float]:
        """Calculate technical debt metrics."""
        if not result.ranked_entities:
            return {}
        
        scores = [score for _, score in result.ranked_entities]
        suggestions = [
            len(vector.refactoring_suggestions or [])
            for vector, _ in result.ranked_entities
        ]
        
        # Calculate a more nuanced debt ratio that accounts for data quality
        # High debt threshold (0.7) for more conservative debt identification
        # Medium debt threshold (0.6) for moderate debt
        # Only count as debt if entity also has refactoring suggestions or very high score
        high_debt_entities = sum(1 for i, score in enumerate(scores) 
                                if score > 0.7 or (score > 0.6 and suggestions[i] > 0))
        
        # Alternative debt calculation - only count entities with actual actionable issues
        actionable_debt = sum(1 for s in suggestions if s > 0)
        
        return {
            "debt_ratio": round(high_debt_entities / len(scores) * 100, 1),
            "actionable_debt_ratio": round(actionable_debt / len(scores) * 100, 1),
            "avg_complexity_score": round(sum(scores) / len(scores), 3),
            "max_complexity_score": round(max(scores), 3),
            "total_refactoring_suggestions": sum(suggestions),
            "avg_suggestions_per_entity": round(sum(suggestions) / len(suggestions), 1),
            "entities_needing_refactoring": sum(1 for s in suggestions if s > 0),
            "high_priority_entities": sum(1 for score in scores if score > 0.7)
        }
    
    def _extract_project_name(self, result: 'PipelineResult') -> str:
        """Extract project name from result."""
        if result.config.roots:
            return Path(result.config.roots[0].path).name
        return "Unknown Project"
    
    def _extract_entity_name(self, entity_id: str) -> str:
        """Extract entity name from entity_id."""
        if ":" in entity_id:
            return entity_id.split(":")[-1]
        return Path(entity_id).name
    
    def _extract_file_path(self, entity_id: str) -> str:
        """Extract file path from entity_id."""
        if ":" in entity_id:
            return entity_id.split(":")[0]
        return entity_id
    
    def _map_to_severity(self, score: float, has_high_severity: bool) -> str:
        """Map complexity score to severity level."""
        if has_high_severity or score > 0.9:
            return SeverityLevel.BLOCKER.value
        elif score > 0.8:
            return SeverityLevel.CRITICAL.value
        elif score > 0.6:
            return SeverityLevel.MAJOR.value
        elif score > 0.3:
            return SeverityLevel.MINOR.value
        else:
            return SeverityLevel.INFO.value
    
    def _extract_primary_issues(self, vector: FeatureVector, scorer: WeightedScorer) -> List[str]:
        """Extract primary issues from feature explanations."""
        explanations = scorer.explain_score(vector)
        primary_issues = []
        
        # Get top 3 contributing features
        feature_contributions = []
        for feature_name, value in vector.normalized_features.items():
            weight = scorer.weights.get(feature_name, 1.0)
            contribution = value * weight
            if contribution > 0.1:  # Significant contribution
                feature_contributions.append((feature_name, contribution, value))
        
        # Sort by contribution
        feature_contributions.sort(key=lambda x: x[1], reverse=True)
        
        # Convert to readable issue descriptions
        feature_to_issue = {
            "cyclomatic_complexity": "High cyclomatic complexity",
            "line_count": "Excessive lines of code",
            "parameter_count": "Too many parameters", 
            "nesting_depth": "Deep nesting levels",
            "cognitive_complexity": "High cognitive complexity",
            "refactoring_urgency": "Urgent refactoring needed",
            "suggestion_count": "Multiple refactoring opportunities",
            "maintainability": "Poor maintainability score"
        }
        
        for feature_name, contribution, value in feature_contributions[:3]:
            issue_desc = feature_to_issue.get(feature_name, f"High {feature_name}")
            primary_issues.append(f"{issue_desc} ({value:.2f})")
        
        return primary_issues
    
    def _export_markdown(self, report: TeamReport, output_path: Path) -> Path:
        """Export structured markdown report with tables and visual indicators."""
        output_file = output_path / "team_report.md"
        
        # Health score emoji
        health_emoji = "🟢" if report.overall_health_score >= 80 else "🟡" if report.overall_health_score >= 60 else "🔴"
        
        markdown_content = f"""# 📊 Code Quality Report: {report.project_name}

**Generated:** {datetime.fromisoformat(report.analysis_date).strftime('%Y-%m-%d %H:%M:%S')}  
**Overall Health Score:** {health_emoji} {report.overall_health_score}/100

---

## 🎯 Executive Summary

| Metric | Value |
|--------|--------|
| **Files Analyzed** | {report.total_files:,} |
| **Code Entities** | {report.total_entities:,} |
| **Processing Time** | {report.processing_time:.2f}s |
| **Priority Issues** | ⚠️ {report.priority_issues_count} |
| **Technical Debt Ratio** | {report.debt_metrics.get('debt_ratio', 0)}% |

---

## 📈 Language Breakdown

| Language | Files | Entities | Avg Score | Max Score | Suggestions | Status |
|----------|--------|----------|-----------|-----------|-------------|---------|"""

        for language, stats in report.language_stats.items():
            status_emoji = "✅" if stats["avg_score"] < 0.5 else "⚠️" if stats["avg_score"] < 0.7 else "❌"
            markdown_content += f"""
| {language} | {stats["file_count"]} | {stats["entity_count"]} | {stats["avg_score"]} | {stats["max_score"]} | {stats["refactoring_suggestions"]} | {status_emoji} |"""

        markdown_content += f"""

---

## 🚨 Critical Issues Requiring Attention

"""
        if not report.critical_issues:
            markdown_content += "*No critical issues found! 🎉*\n"
        else:
            markdown_content += "| Entity | File | Severity | Score | Primary Issues |\n|--------|------|----------|-------|----------------|\n"
            
            for issue in report.critical_issues[:10]:  # Top 10
                severity_emoji = {"BLOCKER": "🚫", "CRITICAL": "🔴", "MAJOR": "🟠", "MINOR": "🟡", "INFO": "🔵"}.get(issue["severity"], "❓")
                entity_name = issue["entity_name"][:30] + "..." if len(issue["entity_name"]) > 30 else issue["entity_name"]
                file_path = issue["file_path"][:25] + "..." if len(issue["file_path"]) > 25 else issue["file_path"]
                primary_issues = ", ".join(issue["primary_issues"][:2])[:50] + "..." if len(", ".join(issue["primary_issues"])) > 50 else ", ".join(issue["primary_issues"][:2])
                
                markdown_content += f"| `{entity_name}` | `{file_path}` | {severity_emoji} {issue['severity']} | {issue['score']:.3f} | {primary_issues} |\n"

        markdown_content += f"""

---

## 🔧 Prioritized Refactoring Recommendations

"""
        if not report.refactoring_recommendations:
            markdown_content += "*No refactoring recommendations available.*\n"
        else:
            for i, rec in enumerate(report.refactoring_recommendations[:8], 1):
                effort_emoji = {"low": "🟢", "medium": "🟡", "high": "🔴"}.get(rec["effort"], "❓")
                markdown_content += f"""
### {i}. {rec["type"].replace("_", " ").title()}

- **Effort:** {effort_emoji} {rec["effort"].title()}
- **Occurrences:** {rec["count"]} entities
- **Average Complexity:** {rec["avg_score"]}
- **Priority Score:** {rec["priority_score"]:.1f}

**Examples:**"""
                
                for example in rec["examples"][:2]:
                    entity_name = example["entity_id"].split(":")[-1] if ":" in example["entity_id"] else example["entity_id"]
                    markdown_content += f"""
- `{entity_name}`: {example["title"]}"""
                
                markdown_content += "\n"

        markdown_content += f"""

---

## 📊 Technical Debt Metrics

| Metric | Value | Target | Status |
|--------|-------|---------|--------|
| **Debt Ratio** | {report.debt_metrics.get('debt_ratio', 0)}% | < 20% | {'✅' if report.debt_metrics.get('debt_ratio', 0) < 20 else '⚠️' if report.debt_metrics.get('debt_ratio', 0) < 40 else '❌'} |
| **Avg Complexity** | {report.debt_metrics.get('avg_complexity_score', 0)} | < 0.5 | {'✅' if report.debt_metrics.get('avg_complexity_score', 0) < 0.5 else '⚠️' if report.debt_metrics.get('avg_complexity_score', 0) < 0.7 else '❌'} |
| **Max Complexity** | {report.debt_metrics.get('max_complexity_score', 0)} | < 0.8 | {'✅' if report.debt_metrics.get('max_complexity_score', 0) < 0.8 else '⚠️' if report.debt_metrics.get('max_complexity_score', 0) < 0.9 else '❌'} |
| **Total Suggestions** | {report.debt_metrics.get('total_refactoring_suggestions', 0)} | - | - |
| **Entities Needing Work** | {report.debt_metrics.get('entities_needing_refactoring', 0)} | - | - |
| **High Priority** | {report.debt_metrics.get('high_priority_entities', 0)} | 0 | {'✅' if report.debt_metrics.get('high_priority_entities', 0) == 0 else '❌'} |

---

## 🎯 Next Steps

1. **Immediate Action Required:**
   - Address {len([i for i in report.critical_issues if i["severity"] in ["BLOCKER", "CRITICAL"]])} critical/blocker issues
   - Focus on entities with complexity score > 0.8

2. **Sprint Planning:**
   - Plan refactoring tasks for top {min(5, len(report.refactoring_recommendations))} recommendation types
   - Allocate ~{sum(1 for r in report.refactoring_recommendations[:5] if r["effort"] == "high") * 8 + sum(1 for r in report.refactoring_recommendations[:5] if r["effort"] == "medium") * 4 + sum(1 for r in report.refactoring_recommendations[:5] if r["effort"] == "low") * 2} story points for technical debt

3. **Long-term Goals:**
   - Target debt ratio below 20%
   - Maintain health score above 80
   - Establish automated quality gates

---

*Generated by [Valknut](https://github.com/yourusername/valknut) - AI-powered code analysis*
"""
        
        with output_file.open("w", encoding="utf-8") as f:
            f.write(markdown_content)
        
        return output_file
    
    def _export_html(self, report: TeamReport, output_path: Path) -> Path:
        """Export professional HTML report with interactive elements."""
        output_file = output_path / "team_report.html"
        
        # Health score color
        health_color = "#28a745" if report.overall_health_score >= 80 else "#ffc107" if report.overall_health_score >= 60 else "#dc3545"
        
        html_content = f"""<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Code Quality Report - {html.escape(report.project_name)}</title>
    <style>
        :root {{
            --primary-color: #007bff;
            --success-color: #28a745;
            --warning-color: #ffc107;
            --danger-color: #dc3545;
            --info-color: #17a2b8;
            --light-color: #f8f9fa;
            --dark-color: #343a40;
            --border-color: #dee2e6;
        }}
        
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            line-height: 1.6;
            color: var(--dark-color);
            background-color: #ffffff;
        }}
        
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            padding: 20px;
        }}
        
        .header {{
            background: linear-gradient(135deg, var(--primary-color), #0056b3);
            color: white;
            padding: 2rem 0;
            margin-bottom: 2rem;
            border-radius: 8px;
            text-align: center;
        }}
        
        .header h1 {{
            font-size: 2.5rem;
            margin-bottom: 0.5rem;
        }}
        
        .header .subtitle {{
            font-size: 1.1rem;
            opacity: 0.9;
        }}
        
        .health-score {{
            display: inline-flex;
            align-items: center;
            background: rgba(255, 255, 255, 0.2);
            padding: 0.5rem 1rem;
            border-radius: 25px;
            margin-top: 1rem;
            font-size: 1.2rem;
            font-weight: bold;
        }}
        
        .health-circle {{
            width: 40px;
            height: 40px;
            border-radius: 50%;
            background: {health_color};
            display: flex;
            align-items: center;
            justify-content: center;
            margin-right: 0.5rem;
            font-size: 0.9rem;
        }}
        
        .summary-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 1rem;
            margin-bottom: 2rem;
        }}
        
        .summary-card {{
            background: white;
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 1.5rem;
            text-align: center;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            transition: transform 0.2s;
        }}
        
        .summary-card:hover {{
            transform: translateY(-2px);
            box-shadow: 0 4px 8px rgba(0,0,0,0.15);
        }}
        
        .summary-card .value {{
            font-size: 2rem;
            font-weight: bold;
            color: var(--primary-color);
            margin-bottom: 0.5rem;
        }}
        
        .summary-card .label {{
            color: #666;
            font-size: 0.9rem;
        }}
        
        .section {{
            background: white;
            border: 1px solid var(--border-color);
            border-radius: 8px;
            margin-bottom: 2rem;
            overflow: hidden;
            box-shadow: 0 2px 4px rgba(0,0,0,0.05);
        }}
        
        .section-header {{
            background: var(--light-color);
            padding: 1rem 1.5rem;
            border-bottom: 1px solid var(--border-color);
        }}
        
        .section-header h2 {{
            font-size: 1.5rem;
            color: var(--dark-color);
            margin: 0;
        }}
        
        .section-content {{
            padding: 1.5rem;
        }}
        
        .table-responsive {{
            overflow-x: auto;
        }}
        
        table {{
            width: 100%;
            border-collapse: collapse;
            margin-bottom: 1rem;
        }}
        
        th, td {{
            padding: 0.75rem;
            text-align: left;
            border-bottom: 1px solid var(--border-color);
        }}
        
        th {{
            background: var(--light-color);
            font-weight: 600;
            color: var(--dark-color);
        }}
        
        tr:hover {{
            background: rgba(0, 123, 255, 0.05);
        }}
        
        .badge {{
            display: inline-block;
            padding: 0.25rem 0.5rem;
            font-size: 0.75rem;
            font-weight: 600;
            border-radius: 0.375rem;
            text-align: center;
            white-space: nowrap;
        }}
        
        .badge-success {{ background-color: var(--success-color); color: white; }}
        .badge-warning {{ background-color: var(--warning-color); color: var(--dark-color); }}
        .badge-danger {{ background-color: var(--danger-color); color: white; }}
        .badge-info {{ background-color: var(--info-color); color: white; }}
        .badge-secondary {{ background-color: #6c757d; color: white; }}
        
        .progress {{
            width: 100%;
            height: 8px;
            background: var(--light-color);
            border-radius: 4px;
            overflow: hidden;
            margin: 0.5rem 0;
        }}
        
        .progress-bar {{
            height: 100%;
            transition: width 0.3s ease;
        }}
        
        .progress-success {{ background-color: var(--success-color); }}
        .progress-warning {{ background-color: var(--warning-color); }}
        .progress-danger {{ background-color: var(--danger-color); }}
        
        .recommendation {{
            border: 1px solid var(--border-color);
            border-radius: 6px;
            margin-bottom: 1rem;
            overflow: hidden;
        }}
        
        .recommendation-header {{
            background: var(--light-color);
            padding: 1rem;
            display: flex;
            justify-content: space-between;
            align-items: center;
            cursor: pointer;
        }}
        
        .recommendation-body {{
            padding: 1rem;
            display: none;
        }}
        
        .recommendation.expanded .recommendation-body {{
            display: block;
        }}
        
        .examples {{
            margin-top: 1rem;
        }}
        
        .example {{
            background: var(--light-color);
            padding: 0.5rem;
            margin: 0.5rem 0;
            border-radius: 4px;
            font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
            font-size: 0.9rem;
        }}
        
        .footer {{
            text-align: center;
            padding: 2rem;
            color: #666;
            border-top: 1px solid var(--border-color);
            margin-top: 3rem;
        }}
        
        @media (max-width: 768px) {{
            .container {{
                padding: 10px;
            }}
            
            .header h1 {{
                font-size: 2rem;
            }}
            
            .summary-grid {{
                grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
                gap: 0.5rem;
            }}
            
            .summary-card {{
                padding: 1rem;
            }}
            
            .summary-card .value {{
                font-size: 1.5rem;
            }}
            
            table {{
                font-size: 0.9rem;
            }}
            
            th, td {{
                padding: 0.5rem;
            }}
        }}
        
        @media print {{
            .recommendation-body {{
                display: block !important;
            }}
            
            .section {{
                break-inside: avoid;
            }}
        }}
    </style>
</head>
<body>
    <div class="container">
        <header class="header">
            <h1>📊 Code Quality Report</h1>
            <p class="subtitle">{html.escape(report.project_name)} - {datetime.fromisoformat(report.analysis_date).strftime('%Y-%m-%d %H:%M:%S')}</p>
            <div class="health-score">
                <div class="health-circle">{report.overall_health_score:.0f}</div>
                Overall Health Score: {report.overall_health_score}/100
            </div>
        </header>

        <div class="summary-grid">
            <div class="summary-card">
                <div class="value">{report.total_files:,}</div>
                <div class="label">Files Analyzed</div>
            </div>
            <div class="summary-card">
                <div class="value">{report.total_entities:,}</div>
                <div class="label">Code Entities</div>
            </div>
            <div class="summary-card">
                <div class="value">{report.processing_time:.2f}s</div>
                <div class="label">Processing Time</div>
            </div>
            <div class="summary-card">
                <div class="value">{report.priority_issues_count}</div>
                <div class="label">Priority Issues</div>
            </div>
        </div>"""

        # Language breakdown
        if report.language_stats:
            html_content += """
        <div class="section">
            <div class="section-header">
                <h2>📈 Language Breakdown</h2>
            </div>
            <div class="section-content">
                <div class="table-responsive">
                    <table>
                        <thead>
                            <tr>
                                <th>Language</th>
                                <th>Files</th>
                                <th>Entities</th>
                                <th>Avg Score</th>
                                <th>Max Score</th>
                                <th>Suggestions</th>
                                <th>Health</th>
                            </tr>
                        </thead>
                        <tbody>"""
            
            for language, stats in report.language_stats.items():
                health_class = "success" if stats["avg_score"] < 0.5 else "warning" if stats["avg_score"] < 0.7 else "danger"
                health_text = "Healthy" if stats["avg_score"] < 0.5 else "Moderate" if stats["avg_score"] < 0.7 else "Needs Attention"
                
                # Progress bar for average score
                progress_width = min(100, stats["avg_score"] * 100)
                progress_class = "progress-success" if stats["avg_score"] < 0.5 else "progress-warning" if stats["avg_score"] < 0.7 else "progress-danger"
                
                html_content += f"""
                            <tr>
                                <td><strong>{html.escape(language)}</strong></td>
                                <td>{stats["file_count"]}</td>
                                <td>{stats["entity_count"]}</td>
                                <td>
                                    {stats["avg_score"]:.3f}
                                    <div class="progress">
                                        <div class="progress-bar {progress_class}" style="width: {progress_width}%"></div>
                                    </div>
                                </td>
                                <td>{stats["max_score"]:.3f}</td>
                                <td>{stats["refactoring_suggestions"]}</td>
                                <td><span class="badge badge-{health_class}">{health_text}</span></td>
                            </tr>"""
            
            html_content += """
                        </tbody>
                    </table>
                </div>
            </div>
        </div>"""

        # Critical issues
        html_content += """
        <div class="section">
            <div class="section-header">
                <h2>🚨 Critical Issues</h2>
            </div>
            <div class="section-content">"""
        
        if not report.critical_issues:
            html_content += '<p class="text-success">🎉 No critical issues found! Your code quality is excellent.</p>'
        else:
            html_content += """
                <div class="table-responsive">
                    <table>
                        <thead>
                            <tr>
                                <th>Entity</th>
                                <th>File</th>
                                <th>Severity</th>
                                <th>Score</th>
                                <th>Primary Issues</th>
                                <th>Suggestions</th>
                            </tr>
                        </thead>
                        <tbody>"""
            
            severity_badge_map = {
                "BLOCKER": "danger",
                "CRITICAL": "danger", 
                "MAJOR": "warning",
                "MINOR": "info",
                "INFO": "secondary"
            }
            
            for issue in report.critical_issues[:15]:  # Top 15
                badge_class = severity_badge_map.get(issue["severity"], "secondary")
                entity_name = html.escape(issue["entity_name"])
                file_path = html.escape(issue["file_path"])
                primary_issues = html.escape(", ".join(issue["primary_issues"][:2]))
                
                html_content += f"""
                            <tr>
                                <td><code>{entity_name}</code></td>
                                <td><code>{file_path}</code></td>
                                <td><span class="badge badge-{badge_class}">{issue['severity']}</span></td>
                                <td>{issue['score']:.3f}</td>
                                <td>{primary_issues}</td>
                                <td>{issue['refactoring_count']}</td>
                            </tr>"""
            
            html_content += """
                        </tbody>
                    </table>
                </div>"""
        
        html_content += """
            </div>
        </div>"""

        # Refactoring recommendations
        html_content += """
        <div class="section">
            <div class="section-header">
                <h2>🔧 Refactoring Recommendations</h2>
            </div>
            <div class="section-content">"""
        
        if not report.refactoring_recommendations:
            html_content += '<p>No specific refactoring recommendations available.</p>'
        else:
            for i, rec in enumerate(report.refactoring_recommendations[:8], 1):
                effort_badge = {"low": "success", "medium": "warning", "high": "danger"}.get(rec["effort"], "secondary")
                rec_type_title = rec["type"].replace("_", " ").title()
                
                html_content += f"""
                <div class="recommendation" onclick="toggleRecommendation(this)">
                    <div class="recommendation-header">
                        <div>
                            <strong>{i}. {html.escape(rec_type_title)}</strong>
                            <span class="badge badge-{effort_badge}">{rec["effort"].title()} Effort</span>
                        </div>
                        <div>
                            <span style="color: #666; font-size: 0.9rem;">
                                {rec["count"]} occurrences • Priority: {rec["priority_score"]:.1f}
                            </span>
                        </div>
                    </div>
                    <div class="recommendation-body">
                        <p><strong>Average Complexity:</strong> {rec["avg_score"]:.3f}</p>
                        <div class="examples">
                            <strong>Examples:</strong>"""
                
                for example in rec["examples"][:3]:
                    entity_name = example["entity_id"].split(":")[-1] if ":" in example["entity_id"] else example["entity_id"]
                    html_content += f"""
                            <div class="example">
                                <strong>{html.escape(entity_name)}:</strong> {html.escape(example["title"])}
                            </div>"""
                
                html_content += """
                        </div>
                    </div>
                </div>"""
        
        html_content += """
            </div>
        </div>"""

        # Technical debt metrics
        debt_ratio = report.debt_metrics.get('debt_ratio', 0)
        avg_complexity = report.debt_metrics.get('avg_complexity_score', 0)
        max_complexity = report.debt_metrics.get('max_complexity_score', 0)
        
        html_content += f"""
        <div class="section">
            <div class="section-header">
                <h2>📊 Technical Debt Metrics</h2>
            </div>
            <div class="section-content">
                <div class="table-responsive">
                    <table>
                        <thead>
                            <tr>
                                <th>Metric</th>
                                <th>Value</th>
                                <th>Target</th>
                                <th>Status</th>
                                <th>Progress</th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td><strong>Debt Ratio</strong></td>
                                <td>{debt_ratio}%</td>
                                <td>&lt; 20%</td>
                                <td><span class="badge badge-{'success' if debt_ratio < 20 else 'warning' if debt_ratio < 40 else 'danger'}">{'Good' if debt_ratio < 20 else 'Warning' if debt_ratio < 40 else 'Critical'}</span></td>
                                <td>
                                    <div class="progress">
                                        <div class="progress-bar {'progress-success' if debt_ratio < 20 else 'progress-warning' if debt_ratio < 40 else 'progress-danger'}" style="width: {min(100, debt_ratio)}%"></div>
                                    </div>
                                </td>
                            </tr>
                            <tr>
                                <td><strong>Avg Complexity</strong></td>
                                <td>{avg_complexity:.3f}</td>
                                <td>&lt; 0.5</td>
                                <td><span class="badge badge-{'success' if avg_complexity < 0.5 else 'warning' if avg_complexity < 0.7 else 'danger'}">{'Good' if avg_complexity < 0.5 else 'Warning' if avg_complexity < 0.7 else 'Critical'}</span></td>
                                <td>
                                    <div class="progress">
                                        <div class="progress-bar {'progress-success' if avg_complexity < 0.5 else 'progress-warning' if avg_complexity < 0.7 else 'progress-danger'}" style="width: {min(100, avg_complexity * 100)}%"></div>
                                    </div>
                                </td>
                            </tr>
                            <tr>
                                <td><strong>Max Complexity</strong></td>
                                <td>{max_complexity:.3f}</td>
                                <td>&lt; 0.8</td>
                                <td><span class="badge badge-{'success' if max_complexity < 0.8 else 'warning' if max_complexity < 0.9 else 'danger'}">{'Good' if max_complexity < 0.8 else 'Warning' if max_complexity < 0.9 else 'Critical'}</span></td>
                                <td>
                                    <div class="progress">
                                        <div class="progress-bar {'progress-success' if max_complexity < 0.8 else 'progress-warning' if max_complexity < 0.9 else 'progress-danger'}" style="width: {min(100, max_complexity * 100)}%"></div>
                                    </div>
                                </td>
                            </tr>
                            <tr>
                                <td><strong>Total Suggestions</strong></td>
                                <td>{report.debt_metrics.get('total_refactoring_suggestions', 0)}</td>
                                <td>-</td>
                                <td><span class="badge badge-info">Info</span></td>
                                <td>-</td>
                            </tr>
                            <tr>
                                <td><strong>High Priority Entities</strong></td>
                                <td>{report.debt_metrics.get('high_priority_entities', 0)}</td>
                                <td>0</td>
                                <td><span class="badge badge-{'success' if report.debt_metrics.get('high_priority_entities', 0) == 0 else 'danger'}">{'Good' if report.debt_metrics.get('high_priority_entities', 0) == 0 else 'Action Needed'}</span></td>
                                <td>-</td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            </div>
        </div>

        <footer class="footer">
            <p>Generated by <strong>Valknut</strong> - AI-powered code analysis • <a href="https://github.com/yourusername/valknut">Learn More</a></p>
        </footer>
    </div>

    <script>
        function toggleRecommendation(element) {{
            element.classList.toggle('expanded');
        }}
        
        // Auto-expand first recommendation
        document.addEventListener('DOMContentLoaded', function() {{
            const firstRec = document.querySelector('.recommendation');
            if (firstRec) {{
                firstRec.classList.add('expanded');
            }}
        }});
    </script>
</body>
</html>"""
        
        with output_file.open("w", encoding="utf-8") as f:
            f.write(html_content)
        
        return output_file
    
    def _export_sonar(self, report: TeamReport, output_path: Path) -> Path:
        """Export SonarQube-compatible JSON format."""
        output_file = output_path / "sonar_issues.json"
        
        sonar_issues = []
        
        for issue in report.critical_issues:
            # Map severity to SonarQube format
            sonar_severity = issue["severity"]
            
            # Extract file path and line information
            file_path = issue["file_path"]
            entity_name = issue["entity_name"]
            
            # Create SonarQube issue
            sonar_issue = {
                "engineId": "valknut",
                "ruleId": "complexity_analysis",
                "type": "CODE_SMELL",
                "severity": sonar_severity,
                "primaryLocation": {
                    "message": f"High complexity detected in {entity_name} (score: {issue['score']:.3f})",
                    "filePath": file_path,
                    "textRange": {
                        "startLine": 1,
                        "endLine": 1,
                        "startColumn": 1,
                        "endColumn": 50
                    }
                },
                "secondaryLocations": [],
                "effortMinutes": self._estimate_effort_minutes(issue, report.refactoring_recommendations)
            }
            
            sonar_issues.append(sonar_issue)
        
        # Add refactoring suggestions as separate issues
        for rec in report.refactoring_recommendations:
            for example in rec["examples"][:1]:  # One issue per recommendation type
                effort_minutes = {"low": 30, "medium": 120, "high": 480}.get(rec["effort"], 120)
                
                file_path = example["entity_id"].split(":")[0] if ":" in example["entity_id"] else "unknown.py"
                
                sonar_issue = {
                    "engineId": "valknut", 
                    "ruleId": f"refactoring_{rec['type']}",
                    "type": "CODE_SMELL",
                    "severity": self._map_refactoring_to_sonar_severity(rec["effort"], rec["avg_score"]),
                    "primaryLocation": {
                        "message": f"{example['title']} - {rec['type'].replace('_', ' ').title()}",
                        "filePath": file_path,
                        "textRange": {
                            "startLine": 1,
                            "endLine": 1,
                            "startColumn": 1,
                            "endColumn": 50
                        }
                    },
                    "secondaryLocations": [],
                    "effortMinutes": effort_minutes
                }
                
                sonar_issues.append(sonar_issue)
        
        sonar_report = {
            "issues": sonar_issues,
            "rules": [
                {
                    "id": "complexity_analysis",
                    "name": "High Complexity Detection",
                    "description": "Detects code entities with high complexity scores that need refactoring",
                    "engineId": "valknut",
                    "type": "CODE_SMELL",
                    "severity": "MAJOR",
                    "tags": ["complexity", "maintainability"]
                }
            ]
        }
        
        # Add refactoring rules
        for rec_type in set(rec["type"] for rec in report.refactoring_recommendations):
            rule_name = rec_type.replace("_", " ").title()
            sonar_report["rules"].append({
                "id": f"refactoring_{rec_type}",
                "name": f"{rule_name} Opportunity",
                "description": f"Suggests {rule_name.lower()} refactoring to improve code quality",
                "engineId": "valknut",
                "type": "CODE_SMELL",
                "severity": "MINOR",
                "tags": ["refactoring", "maintainability"]
            })
        
        with output_file.open("w", encoding="utf-8") as f:
            json.dump(sonar_report, f, indent=2)
        
        return output_file
    
    def _export_csv(self, report: TeamReport, output_path: Path) -> Path:
        """Export CSV for team dashboards and spreadsheet analysis."""
        output_file = output_path / "analysis_data.csv"
        
        with output_file.open("w", newline="", encoding="utf-8") as f:
            writer = csv.writer(f)
            
            # Header
            writer.writerow([
                "Entity ID",
                "Entity Name", 
                "File Path",
                "Language",
                "Complexity Score",
                "Severity",
                "Primary Issues",
                "Refactoring Count",
                "Effort Estimate (hours)",
                "Priority Score",
                "Recommendations"
            ])
            
            # Data rows
            for issue in report.critical_issues:
                # Extract language from file extension
                file_path = issue["file_path"]
                extension = Path(file_path).suffix.lower()
                lang_map = {
                    ".py": "Python", ".js": "JavaScript", ".ts": "TypeScript",
                    ".jsx": "JavaScript (JSX)", ".tsx": "TypeScript (JSX)",
                    ".rs": "Rust", ".go": "Go", ".java": "Java",
                    ".cpp": "C++", ".c": "C", ".cs": "C#", ".rb": "Ruby", ".php": "PHP"
                }
                language = lang_map.get(extension, "Unknown")
                
                # Estimate effort in hours based on severity and refactoring count
                effort_map = {"BLOCKER": 8, "CRITICAL": 6, "MAJOR": 4, "MINOR": 2, "INFO": 1}
                base_effort = effort_map.get(issue["severity"], 2)
                effort_hours = base_effort + (issue["refactoring_count"] * 0.5)
                
                # Priority score based on severity and complexity
                priority_map = {"BLOCKER": 100, "CRITICAL": 80, "MAJOR": 60, "MINOR": 40, "INFO": 20}
                priority_score = priority_map.get(issue["severity"], 20) + (issue["score"] * 20)
                
                # Recommendations summary
                rec_summary = "; ".join([s["title"] for s in issue["high_priority_suggestions"][:3]])
                
                writer.writerow([
                    issue["entity_id"],
                    issue["entity_name"],
                    issue["file_path"],
                    language,
                    f"{issue['score']:.3f}",
                    issue["severity"],
                    "; ".join(issue["primary_issues"]),
                    issue["refactoring_count"],
                    f"{effort_hours:.1f}",
                    f"{priority_score:.1f}",
                    rec_summary
                ])
        
        return output_file
    
    def _estimate_effort_minutes(self, issue: Dict[str, Any], recommendations: List[Dict[str, Any]]) -> int:
        """Estimate effort in minutes for SonarQube integration."""
        base_minutes = {"BLOCKER": 480, "CRITICAL": 240, "MAJOR": 120, "MINOR": 60, "INFO": 30}
        base = base_minutes.get(issue["severity"], 60)
        
        # Add minutes based on refactoring suggestions
        additional = len(issue.get("high_priority_suggestions", [])) * 30
        
        return min(600, base + additional)  # Cap at 10 hours
    
    def _map_refactoring_to_sonar_severity(self, effort: str, avg_score: float) -> str:
        """Map refactoring effort and score to SonarQube severity."""
        if effort == "high" or avg_score > 0.8:
            return SeverityLevel.MAJOR.value
        elif effort == "medium" or avg_score > 0.6:
            return SeverityLevel.MINOR.value
        else:
            return SeverityLevel.INFO.value