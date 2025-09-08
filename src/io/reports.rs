//! Report generation with template engine support.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use handlebars::Handlebars;
use serde_json::Value;
use thiserror::Error;
use chrono::{DateTime, Utc};
use crate::core::config::ReportFormat;
use crate::api::results::AnalysisResults;

#[derive(Error, Debug)]
pub enum ReportError {
    #[error("Template error: {0}")]
    Template(#[from] handlebars::TemplateError),
    #[error("Render error: {0}")]
    Render(#[from] handlebars::RenderError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct ReportGenerator {
    handlebars: Handlebars<'static>,
    templates_dir: Option<PathBuf>,
}

impl Default for ReportGenerator {
    fn default() -> Self {
        let mut handlebars = Handlebars::new();
        
        // Register built-in templates
        if let Err(e) = handlebars.register_template_string("default_html", DEFAULT_HTML_TEMPLATE) {
            eprintln!("Failed to register default HTML template: {}", e);
        }
        
        Self {
            handlebars,
            templates_dir: None,
        }
    }
}

impl ReportGenerator {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_templates_dir<P: AsRef<Path>>(mut self, templates_dir: P) -> Result<Self, ReportError> {
        let templates_dir = templates_dir.as_ref().to_path_buf();
        
        if templates_dir.exists() {
            // Load custom templates from directory
            self.load_templates_from_dir(&templates_dir)?;
        }
        
        self.templates_dir = Some(templates_dir);
        Ok(self)
    }
    
    fn load_templates_from_dir<P: AsRef<Path>>(&mut self, templates_dir: P) -> Result<(), ReportError> {
        let templates_dir = templates_dir.as_ref();
        
        for entry in fs::read_dir(templates_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("hbs") {
                let template_name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .ok_or_else(|| std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid template filename"
                    ))?;
                
                let template_content = fs::read_to_string(&path)?;
                self.handlebars.register_template_string(template_name, template_content)?;
            }
        }
        
        Ok(())
    }
    
    pub fn generate_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
        format: ReportFormat,
    ) -> Result<(), ReportError> {
        match format {
            ReportFormat::Html => self.generate_html_report(results, output_path),
            ReportFormat::Json => self.generate_json_report(results, output_path),
            ReportFormat::Yaml => self.generate_yaml_report(results, output_path),
            ReportFormat::Csv => self.generate_csv_report(results, output_path),
        }
    }
    
    fn generate_html_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        let template_data = self.prepare_template_data(results);
        
        // Use custom template if available, otherwise use default
        let template_name = if self.handlebars.get_templates().contains_key("report") {
            "report"
        } else {
            "default_html"
        };
        
        let html_content = self.handlebars.render(template_name, &template_data)?;
        fs::write(output_path, html_content)?;
        
        Ok(())
    }
    
    fn generate_json_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        let json_content = serde_json::to_string_pretty(results)?;
        fs::write(output_path, json_content)?;
        Ok(())
    }
    
    fn generate_yaml_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        let yaml_content = serde_yaml::to_string(results)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        fs::write(output_path, yaml_content)?;
        Ok(())
    }
    
    fn generate_csv_report<P: AsRef<Path>>(
        &self,
        _results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        // CSV implementation would go here
        // For now, just create a placeholder
        fs::write(output_path, "CSV report not yet implemented")?;
        Ok(())
    }
    
    fn prepare_template_data(&self, results: &AnalysisResults) -> Value {
        let mut data = HashMap::new();
        
        // Add metadata
        data.insert("generated_at", serde_json::to_value(Utc::now().to_rfc3339()).unwrap());
        data.insert("tool_name", serde_json::to_value("Valknut").unwrap());
        data.insert("version", serde_json::to_value(env!("CARGO_PKG_VERSION")).unwrap());
        
        // Add analysis results
        data.insert("results", serde_json::to_value(results).unwrap());
        
        // Add summary statistics
        if let Ok(summary) = serde_json::to_value(self.calculate_summary(results)) {
            data.insert("summary", summary);
        }
        
        serde_json::to_value(data).unwrap_or_else(|_| serde_json::Value::Null)
    }
    
    fn calculate_summary(&self, results: &AnalysisResults) -> HashMap<String, Value> {
        let mut summary = HashMap::new();
        
        // Calculate basic statistics
        summary.insert("total_files".to_string(), serde_json::to_value(results.files_analyzed()).unwrap());
        summary.insert("total_issues".to_string(), serde_json::to_value(results.refactoring_candidates.len()).unwrap());
        
        summary
    }
}

const DEFAULT_HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{tool_name}} Analysis Report</title>
    <style>
        :root {
            --primary-color: #2563eb;
            --secondary-color: #64748b;
            --success-color: #10b981;
            --warning-color: #f59e0b;
            --error-color: #ef4444;
            --background-color: #ffffff;
            --surface-color: #f8fafc;
            --text-primary: #1e293b;
            --text-secondary: #64748b;
            --border-color: #e2e8f0;
        }
        
        * {
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }
        
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Roboto', sans-serif;
            line-height: 1.6;
            color: var(--text-primary);
            background-color: var(--background-color);
        }
        
        .container {
            max-width: 1200px;
            margin: 0 auto;
            padding: 2rem;
        }
        
        .header {
            text-align: center;
            margin-bottom: 3rem;
            padding: 2rem 0;
            border-bottom: 1px solid var(--border-color);
        }
        
        .header h1 {
            color: var(--primary-color);
            font-size: 2.5rem;
            margin-bottom: 0.5rem;
        }
        
        .header .meta {
            color: var(--text-secondary);
            font-size: 0.9rem;
        }
        
        .summary {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 1.5rem;
            margin-bottom: 3rem;
        }
        
        .summary-card {
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 1.5rem;
            text-align: center;
        }
        
        .summary-card .value {
            font-size: 2rem;
            font-weight: bold;
            color: var(--primary-color);
            margin-bottom: 0.5rem;
        }
        
        .summary-card .label {
            color: var(--text-secondary);
            font-size: 0.9rem;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }
        
        .results-section {
            margin-bottom: 3rem;
        }
        
        .results-section h2 {
            color: var(--text-primary);
            margin-bottom: 1.5rem;
            padding-bottom: 0.5rem;
            border-bottom: 2px solid var(--primary-color);
        }
        
        .file-list {
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            overflow: hidden;
        }
        
        .file-item {
            padding: 1rem;
            border-bottom: 1px solid var(--border-color);
            cursor: pointer;
            transition: background-color 0.2s;
        }
        
        .file-item:hover {
            background-color: rgba(37, 99, 235, 0.05);
        }
        
        .file-item:last-child {
            border-bottom: none;
        }
        
        .file-path {
            font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
            color: var(--primary-color);
            font-weight: 500;
            margin-bottom: 0.25rem;
        }
        
        .file-details {
            font-size: 0.9rem;
            color: var(--text-secondary);
        }
        
        .raw-data {
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 1.5rem;
            margin-top: 2rem;
        }
        
        .raw-data h3 {
            margin-bottom: 1rem;
            color: var(--text-primary);
        }
        
        .raw-data pre {
            background: #1e293b;
            color: #e2e8f0;
            padding: 1rem;
            border-radius: 4px;
            overflow-x: auto;
            font-size: 0.85rem;
            line-height: 1.4;
        }
        
        @media (max-width: 768px) {
            .container {
                padding: 1rem;
            }
            
            .header h1 {
                font-size: 2rem;
            }
            
            .summary {
                grid-template-columns: 1fr;
            }
        }
    </style>
</head>
<body>
    <div class="container">
        <header class="header">
            <h1>{{tool_name}} Analysis Report</h1>
            <div class="meta">
                Generated on {{generated_at}} | Version {{version}}
            </div>
        </header>
        
        <section class="summary">
            <div class="summary-card">
                <div class="value">{{summary.total_files}}</div>
                <div class="label">Files Analyzed</div>
            </div>
            <div class="summary-card">
                <div class="value">{{summary.total_issues}}</div>
                <div class="label">Issues Found</div>
            </div>
        </section>
        
        <section class="results-section">
            <h2>Analysis Results</h2>
            <div class="file-list">
                {{#each results.files}}
                <div class="file-item" data-file-path="{{this.path}}">
                    <div class="file-path">{{this.path}}</div>
                    <div class="file-details">
                        Size: {{this.size}} bytes
                    </div>
                </div>
                {{/each}}
            </div>
        </section>
        
        <section class="raw-data">
            <h3>Raw Data</h3>
            <pre><code>{{json results}}</code></pre>
        </section>
    </div>
    
    <script>
        // Add click handlers for file navigation
        document.addEventListener('DOMContentLoaded', function() {
            const fileItems = document.querySelectorAll('.file-item');
            fileItems.forEach(item => {
                item.addEventListener('click', function() {
                    const filePath = this.dataset.filePath;
                    if (window.vscode) {
                        // VS Code extension will handle this
                        window.vscode.postMessage({
                            command: 'openFile',
                            filePath: filePath
                        });
                    } else {
                        // Fallback for web view
                        console.log('Open file:', filePath);
                    }
                });
            });
        });
    </script>
</body>
</html>
"#;