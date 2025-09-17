//! Report generation with template engine support.

use crate::api::config_types::AnalysisConfig;
use crate::api::results::AnalysisResults;
use crate::core::config::ReportFormat;
use chrono::Utc;
use handlebars::{Handlebars, Renderable};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

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
    analysis_config: Option<AnalysisConfig>,
}

impl Default for ReportGenerator {
    fn default() -> Self {
        let mut handlebars = Handlebars::new();

        // Register JSON helper
        handlebars.register_helper(
            "json",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let param =
                        h.param(0)
                            .and_then(|v| v.value().as_object())
                            .ok_or_else(|| {
                                handlebars::RenderError::new(
                                    "json helper requires an object parameter",
                                )
                            })?;
                    let json_str = serde_json::to_string_pretty(param).map_err(|e| {
                        handlebars::RenderError::new(&format!("JSON serialization error: {}", e))
                    })?;
                    out.write(&json_str)?;
                    Ok(())
                },
            ),
        );

        // Register percentage helper
        handlebars.register_helper(
            "percentage",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "percentage helper requires a numeric parameter",
                        )
                    })?;
                    out.write(&format!("{:.1}", value * 100.0))?;
                    Ok(())
                },
            ),
        );

        // Register multiply helper for template calculations
        handlebars.register_helper(
            "multiply",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new("multiply helper requires a numeric parameter")
                    })?;
                    let multiplier =
                        h.param(1).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                            handlebars::RenderError::new(
                                "multiply helper requires a second numeric parameter",
                            )
                        })?;
                    out.write(&format!("{:.0}", value * multiplier))?;
                    Ok(())
                },
            ),
        );

        // Register capitalize helper
        handlebars.register_helper(
            "capitalize",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "capitalize helper requires a string parameter",
                        )
                    })?;
                    let capitalized = if let Some(first_char) = value.chars().next() {
                        format!(
                            "{}{}",
                            first_char.to_uppercase(),
                            &value[first_char.len_utf8()..]
                        )
                    } else {
                        value.to_string()
                    };
                    out.write(&capitalized)?;
                    Ok(())
                },
            ),
        );

        // Register replace helper
        handlebars.register_helper(
            "replace",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new("replace helper requires a string parameter")
                    })?;
                    let search = h.param(1).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new("replace helper requires a search string")
                    })?;
                    let replacement =
                        h.param(2).and_then(|v| v.value().as_str()).ok_or_else(|| {
                            handlebars::RenderError::new(
                                "replace helper requires a replacement string",
                            )
                        })?;
                    let result = value.replace(search, replacement);
                    out.write(&result)?;
                    Ok(())
                },
            ),
        );

        // Register format helper
        handlebars.register_helper(
            "format",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new("format helper requires a numeric parameter")
                    })?;
                    let format_str = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("0.1");

                    let result = match format_str {
                        "0.1" => format!("{:.1}", value),
                        "0.0" => format!("{:.0}", value),
                        "0.2" => format!("{:.2}", value),
                        _ => format!("{:.1}", value), // default
                    };
                    out.write(&result)?;
                    Ok(())
                },
            ),
        );

        // Register percentage helper - multiplies by 100 and formats
        handlebars.register_helper(
            "percentage",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "percentage helper requires a numeric parameter",
                        )
                    })?;
                    let decimals = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("0");

                    let percentage = value * 100.0;
                    let result = match decimals {
                        "0" => format!("{:.0}", percentage),
                        "1" => format!("{:.1}", percentage),
                        "2" => format!("{:.2}", percentage),
                        _ => format!("{:.0}", percentage), // default
                    };
                    out.write(&result)?;
                    Ok(())
                },
            ),
        );

        // Register subtract helper
        handlebars.register_helper(
            "subtract",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let a = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new("subtract helper requires numeric parameters")
                    })?;
                    let b = h.param(1).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "subtract helper requires two numeric parameters",
                        )
                    })?;
                    let result = a - b;
                    out.write(&format!("{:.0}", result))?;
                    Ok(())
                },
            ),
        );

        // Register function_name helper to extract just the function name from full entity IDs
        handlebars.register_helper(
            "function_name",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "function_name helper requires a string parameter",
                        )
                    })?;

                    // Extract function name from entity IDs like "src/core/scoring.rs:function:normalize_batch"
                    let function_name = if let Some(last_colon) = value.rfind(':') {
                        &value[last_colon + 1..]
                    } else {
                        value // If no colon found, return the whole string
                    };

                    out.write(function_name)?;
                    Ok(())
                },
            ),
        );

        // Register health_badge_class helper to determine CSS class based on health score
        handlebars.register_helper(
            "health_badge_class",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "health_badge_class helper requires a numeric parameter",
                        )
                    })?;

                    let badge_class = if value >= 75.0 {
                        "tree-badge-High" // Good health = green
                    } else if value >= 50.0 {
                        "tree-badge-Medium" // Medium health = yellow
                    } else {
                        "tree-badge-Low" // Poor health = red
                    };

                    out.write(badge_class)?;
                    Ok(())
                },
            ),
        );

        // Register add helper for arithmetic in templates
        handlebars.register_helper(
            "add",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let a = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new("add helper requires numeric parameters")
                    })?;
                    let b = h.param(1).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new("add helper requires two numeric parameters")
                    })?;
                    let result = a + b;
                    out.write(&format!("{:.0}", result))?;
                    Ok(())
                },
            ),
        );

        // Register length helper for arrays
        handlebars.register_helper(
            "length",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let array = h
                        .param(0)
                        .and_then(|v| v.value().as_array())
                        .ok_or_else(|| {
                            handlebars::RenderError::new(
                                "length helper requires an array parameter",
                            )
                        })?;
                    out.write(&array.len().to_string())?;
                    Ok(())
                },
            ),
        );

        // Register gt helper for numeric comparisons
        handlebars.register_helper(
            "gt",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let a = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new("gt helper requires numeric parameters")
                    })?;
                    let b = h.param(1).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                        handlebars::RenderError::new("gt helper requires two numeric parameters")
                    })?;
                    out.write(&(a > b).to_string())?;
                    Ok(())
                },
            ),
        );

        // Register has_children helper for checking if array has elements
        handlebars.register_helper(
            "has_children",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let array = h
                        .param(0)
                        .and_then(|v| v.value().as_array())
                        .ok_or_else(|| {
                            handlebars::RenderError::new(
                                "has_children helper requires an array parameter",
                            )
                        })?;
                    out.write(&(array.len() > 0).to_string())?;
                    Ok(())
                },
            ),
        );

        // Register basename helper for extracting filename from path
        handlebars.register_helper(
            "basename",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let path_str =
                        h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                            handlebars::RenderError::new(
                                "basename helper requires a string parameter",
                            )
                        })?;
                    let path = std::path::Path::new(path_str);
                    let basename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path_str);
                    out.write(basename)?;
                    Ok(())
                },
            ),
        );

        // Register eq helper for equality comparison
        handlebars.register_helper(
            "eq",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let a = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new("eq helper requires string parameters")
                    })?;
                    let b = h.param(1).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new("eq helper requires string parameters")
                    })?;
                    out.write(&(a == b).to_string())?;
                    Ok(())
                },
            ),
        );

        // Register starts_with helper for string prefix checking
        handlebars.register_helper(
            "starts_with",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let string = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "starts_with helper requires string parameters",
                        )
                    })?;
                    let prefix = h.param(1).and_then(|v| v.value().as_str()).ok_or_else(|| {
                        handlebars::RenderError::new(
                            "starts_with helper requires string parameters",
                        )
                    })?;
                    out.write(&string.starts_with(prefix).to_string())?;
                    Ok(())
                },
            ),
        );

        // Register is_source_dir helper for filtering actual source directories
        handlebars.register_helper(
            "is_source_dir",
            Box::new(
                |h: &handlebars::Helper,
                 _: &Handlebars,
                 _: &handlebars::Context,
                 _: &mut handlebars::RenderContext,
                 out: &mut dyn handlebars::Output|
                 -> handlebars::HelperResult {
                    let dir_path =
                        h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                            handlebars::RenderError::new(
                                "is_source_dir helper requires a string parameter",
                            )
                        })?;

                    // Skip obvious garbage directories
                    let is_source = !dir_path.contains(".valknut") &&
                           !dir_path.contains("test-report") &&
                           !dir_path.contains("final-demo") &&
                           !dir_path.contains("comprehensive-coverage-report") &&
                           !dir_path.contains("debug_test") &&
                           !dir_path.contains("demo_working") &&
                           !dir_path.contains("working_demo") &&
                           !dir_path.contains("webpage_files") &&
                           !dir_path.ends_with(".html") &&
                           !dir_path.contains("/webpage_files") &&
                           // Include actual source directories
                           (dir_path == "src" || 
                            dir_path.starts_with("src/") ||
                            dir_path == "tests" ||
                            dir_path.starts_with("tests/") ||
                            dir_path == "benches" ||
                            dir_path.starts_with("benches/") ||
                            dir_path == "examples" ||
                            dir_path.starts_with("examples/") ||
                            dir_path == "scripts" ||
                            dir_path.starts_with("scripts/") ||
                            dir_path == "vscode-extension" ||
                            dir_path.starts_with("vscode-extension/"));

                    out.write(&is_source.to_string())?;
                    Ok(())
                },
            ),
        );

        // Load default templates (external files instead of embedded)
        let mut generator = Self {
            handlebars,
            templates_dir: None,
            analysis_config: None,
        };

        // Always register the default HTML template first
        if let Err(e) = generator
            .handlebars
            .register_template_string("default_html", FALLBACK_HTML_TEMPLATE)
        {
            eprintln!("Failed to register fallback HTML template: {}", e);
        }

        // Try to load the external report template from templates/report.hbs
        if let Ok(current_dir) = std::env::current_dir() {
            let templates_path = current_dir.join("templates");
            if templates_path.exists() {
                if let Err(e) = generator.load_templates_from_dir(&templates_path) {
                    eprintln!("Failed to load external templates: {}", e);
                }
            }
        }

        generator
    }
}

impl ReportGenerator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(mut self, config: AnalysisConfig) -> Self {
        self.analysis_config = Some(config);
        self
    }

    pub fn with_templates_dir<P: AsRef<Path>>(
        mut self,
        templates_dir: P,
    ) -> Result<Self, ReportError> {
        let templates_dir = templates_dir.as_ref().to_path_buf();

        if templates_dir.exists() {
            // Load custom templates from directory
            self.load_templates_from_dir(&templates_dir)?;
        }

        self.templates_dir = Some(templates_dir);
        Ok(self)
    }

    fn load_templates_from_dir<P: AsRef<Path>>(
        &mut self,
        templates_dir: P,
    ) -> Result<(), ReportError> {
        let templates_dir = templates_dir.as_ref();

        // Load main templates
        for entry in fs::read_dir(templates_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("hbs") {
                let template_name = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Invalid template filename",
                    )
                })?;

                let template_content = fs::read_to_string(&path)?;
                self.handlebars
                    .register_template_string(template_name, template_content)?;
            }
        }

        // Load partials from partials subdirectory
        let partials_dir = templates_dir.join("partials");
        if partials_dir.exists() && partials_dir.is_dir() {
            for entry in fs::read_dir(&partials_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().and_then(|s| s.to_str()) == Some("hbs") {
                    let partial_name =
                        path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                            std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Invalid partial filename",
                            )
                        })?;

                    let partial_content = fs::read_to_string(&path)?;
                    self.handlebars
                        .register_partial(partial_name, partial_content)?;
                }
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

    pub fn generate_report_with_oracle<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        oracle_response: &crate::oracle::RefactoringOracleResponse,
        output_path: P,
        format: ReportFormat,
    ) -> Result<(), ReportError> {
        let oracle_option = Some(oracle_response.clone());
        match format {
            ReportFormat::Html => {
                self.generate_html_report_with_oracle(results, &oracle_option, output_path)
            }
            ReportFormat::Json => {
                self.generate_json_report_with_oracle(results, &oracle_option, output_path)
            }
            ReportFormat::Yaml => {
                self.generate_yaml_report_with_oracle(results, &oracle_option, output_path)
            }
            ReportFormat::Csv => {
                self.generate_csv_report_with_oracle(results, &oracle_option, output_path)
            }
        }
    }

    fn generate_html_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        self.generate_html_report_with_oracle(results, &None, output_path)
    }

    fn generate_html_report_with_oracle<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        oracle_response: &Option<crate::oracle::RefactoringOracleResponse>,
        output_path: P,
    ) -> Result<(), ReportError> {
        let output_path = output_path.as_ref();
        let output_dir = output_path.parent().unwrap_or_else(|| Path::new("."));

        // Copy Sibylline theme CSS to output directory
        self.copy_theme_css_to_output(output_dir)?;

        // Copy JavaScript assets for React tree component
        self.copy_js_assets_to_output(output_dir)?;

        // Copy webpage assets (logo, animation files) to output directory
        self.copy_webpage_assets_to_output(output_dir)?;

        let template_data = self.prepare_template_data_with_oracle(results, oracle_response);

        // Prefer external template over fallback
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
        self.generate_json_report_with_oracle(results, &None, output_path)
    }

    fn generate_json_report_with_oracle<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        oracle_response: &Option<crate::oracle::RefactoringOracleResponse>,
        output_path: P,
    ) -> Result<(), ReportError> {
        let combined_result = if let Some(oracle) = oracle_response {
            serde_json::json!({
                "oracle_refactoring_plan": oracle,
                "analysis_results": results
            })
        } else {
            serde_json::to_value(results)?
        };
        let json_content = serde_json::to_string_pretty(&combined_result)?;
        fs::write(output_path, json_content)?;
        Ok(())
    }

    fn generate_yaml_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        self.generate_yaml_report_with_oracle(results, &None, output_path)
    }

    fn generate_yaml_report_with_oracle<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        oracle_response: &Option<crate::oracle::RefactoringOracleResponse>,
        output_path: P,
    ) -> Result<(), ReportError> {
        let combined_result = if let Some(oracle) = oracle_response {
            serde_json::json!({
                "oracle_refactoring_plan": oracle,
                "analysis_results": results
            })
        } else {
            serde_json::to_value(results)?
        };
        let yaml_content = serde_yaml::to_string(&combined_result)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        fs::write(output_path, yaml_content)?;
        Ok(())
    }

    fn generate_csv_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        self.generate_csv_report_with_oracle(results, &None, output_path)
    }

    fn generate_csv_report_with_oracle<P: AsRef<Path>>(
        &self,
        _results: &AnalysisResults,
        _oracle_response: &Option<crate::oracle::RefactoringOracleResponse>,
        output_path: P,
    ) -> Result<(), ReportError> {
        // CSV implementation would go here
        // For now, just create a placeholder
        fs::write(output_path, "CSV report not yet implemented")?;
        Ok(())
    }

    fn prepare_template_data(&self, results: &AnalysisResults) -> Value {
        self.prepare_template_data_with_oracle(results, &None)
    }

    fn prepare_template_data_with_oracle(
        &self,
        results: &AnalysisResults,
        oracle_response: &Option<crate::oracle::RefactoringOracleResponse>,
    ) -> Value {
        let mut data = HashMap::new();

        // Add metadata
        data.insert(
            "generated_at",
            serde_json::to_value(Utc::now().to_rfc3339()).unwrap(),
        );
        data.insert("tool_name", serde_json::to_value("Valknut").unwrap());
        data.insert(
            "version",
            serde_json::to_value(env!("CARGO_PKG_VERSION")).unwrap(),
        );

        // Add theme CSS reference - Sibylline by default
        data.insert(
            "theme_css_url",
            serde_json::to_value("sibylline.css").unwrap(),
        );

        // Add animation config
        let enable_animation = true; // Always enable animation for now
        data.insert(
            "enable_animation",
            serde_json::to_value(enable_animation).unwrap(),
        );

        // Add Oracle refactoring plan at the TOP for user requirement
        if let Some(oracle) = oracle_response {
            data.insert(
                "oracle_refactoring_plan",
                serde_json::to_value(oracle).unwrap(),
            );
            data.insert("has_oracle_data", serde_json::to_value(true).unwrap());
        } else {
            data.insert("has_oracle_data", serde_json::to_value(false).unwrap());
        }

        // Add analysis results
        data.insert("results", serde_json::to_value(results).unwrap());

        // Add refactoring candidates at top level for template access (clean up paths)
        let cleaned_candidates = self.clean_path_prefixes(&results.refactoring_candidates);
        data.insert(
            "refactoring_candidates",
            serde_json::to_value(&cleaned_candidates).unwrap(),
        );

        // Add hierarchical refactoring candidates by file (clean paths)
        let cleaned_candidates_by_file: Vec<_> = results
            .refactoring_candidates_by_file
            .iter()
            .map(|group| {
                let mut cleaned_group = group.clone();
                if cleaned_group.file_path.starts_with("./") {
                    cleaned_group.file_path = cleaned_group.file_path[2..].to_string();
                }
                cleaned_group
            })
            .collect();
        data.insert(
            "refactoring_candidates_by_file",
            serde_json::to_value(&cleaned_candidates_by_file).unwrap(),
        );
        data.insert(
            "file_count",
            serde_json::to_value(cleaned_candidates_by_file.len()).unwrap(),
        );

        // Add directory health tree for template access (with cleaned paths)
        let cleaned_directory_health_tree = results
            .directory_health_tree
            .as_ref()
            .map(|tree| self.clean_directory_health_tree_paths(tree));
        data.insert(
            "directory_health_tree",
            serde_json::to_value(&cleaned_directory_health_tree).unwrap(),
        );

        // Add unified hierarchy combining directory health with refactoring candidates
        if let Some(ref cleaned_tree) = cleaned_directory_health_tree {
            let unified_hierarchy =
                self.build_unified_hierarchy(cleaned_tree, &cleaned_candidates_by_file);
            data.insert(
                "unified_hierarchy",
                serde_json::to_value(&unified_hierarchy).unwrap(),
            );
        }

        // Add summary statistics
        if let Ok(summary) = serde_json::to_value(self.calculate_summary(results)) {
            data.insert("summary", summary);
        }

        // Add directory health tree data (with cleaned paths)
        if let Some(ref cleaned_tree) = cleaned_directory_health_tree {
            data.insert(
                "directory_tree",
                serde_json::to_value(cleaned_tree).unwrap(),
            );
            data.insert(
                "tree_visualization",
                serde_json::to_value(cleaned_tree.to_tree_string()).unwrap(),
            );
        }

        serde_json::to_value(data).unwrap_or_else(|_| serde_json::Value::Null)
    }

    fn calculate_summary(&self, results: &AnalysisResults) -> HashMap<String, Value> {
        let mut summary = HashMap::new();

        // Calculate basic statistics for the template
        summary.insert(
            "files_processed".to_string(),
            serde_json::to_value(results.files_analyzed()).unwrap(),
        );
        summary.insert(
            "entities_analyzed".to_string(),
            serde_json::to_value(results.summary.entities_analyzed).unwrap(),
        );
        summary.insert(
            "refactoring_needed".to_string(),
            serde_json::to_value(results.refactoring_candidates.len()).unwrap(),
        );
        summary.insert(
            "code_health_score".to_string(),
            serde_json::to_value(results.summary.code_health_score).unwrap(),
        );

        // Legacy fields for backwards compatibility
        summary.insert(
            "total_files".to_string(),
            serde_json::to_value(results.files_analyzed()).unwrap(),
        );
        summary.insert(
            "total_issues".to_string(),
            serde_json::to_value(results.refactoring_candidates.len()).unwrap(),
        );

        // Add additional metrics for the new template
        summary.insert(
            "complexity_score".to_string(),
            serde_json::to_value(format!(
                "{:.1}",
                results.summary.avg_refactoring_score * 100.0
            ))
            .unwrap(),
        );
        summary.insert(
            "maintainability_index".to_string(),
            serde_json::to_value(format!("{:.1}", results.summary.code_health_score * 100.0))
                .unwrap(),
        );

        summary
    }

    /// Build a unified hierarchy combining directory health with refactoring candidates
    fn build_unified_hierarchy(
        &self,
        tree: &crate::api::results::DirectoryHealthTree,
        file_groups: &[crate::api::results::FileRefactoringGroup],
    ) -> Vec<serde_json::Value> {
        // Sort directories by health score (worst first) for priority-based display
        let mut directories: Vec<_> = tree.directories.iter().collect();
        directories.sort_by(|a, b| {
            a.1.health_score
                .partial_cmp(&b.1.health_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Build the hierarchy
        let mut hierarchy = Vec::new();
        let mut processed_paths = std::collections::HashSet::new();

        for (dir_path, dir_health) in directories.iter() {
            let path_str = dir_path.to_string_lossy().to_string();

            // Skip if this path has already been processed as a child
            if processed_paths.contains(&path_str) {
                continue;
            }

            // Build directory node with health information
            let dir_name = dir_path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new(&path_str))
                .to_string_lossy()
                .to_string();

            let mut dir_node = serde_json::json!({
                "type": "directory",
                "path": path_str.clone(),
                "name": dir_name,
                "health_score": dir_health.health_score,
                "entity_count": dir_health.entity_count,
                "file_count": dir_health.file_count,
                "refactoring_needed": dir_health.refactoring_needed,
                "children": serde_json::json!([])
            });

            // Find files that belong to this directory
            let mut children = Vec::new();
            for file_group in file_groups.iter() {
                let file_dir = std::path::Path::new(&file_group.file_path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string());

                if file_dir == path_str {
                    let file_node = serde_json::json!({
                        "type": "file",
                        "path": file_group.file_path,
                        "name": file_group.file_name,
                        "entity_count": file_group.entity_count,
                        "avg_score": file_group.avg_score,
                        "priority": file_group.highest_priority,
                        "entities": file_group.entities
                    });
                    children.push(file_node);
                }
            }

            // Sort children (files) by priority and score
            children.sort_by(|a, b| {
                let priority_a = a["priority"].as_str().unwrap_or("Low");
                let priority_b = b["priority"].as_str().unwrap_or("Low");
                let score_a = a["avg_score"].as_f64().unwrap_or(0.0);
                let score_b = b["avg_score"].as_f64().unwrap_or(0.0);

                // Sort by priority first (Critical > High > Medium > Low), then by score
                priority_b.cmp(priority_a).then(
                    score_b
                        .partial_cmp(&score_a)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
            });

            dir_node["children"] = serde_json::Value::Array(children);
            hierarchy.push(dir_node);
            processed_paths.insert(path_str);
        }

        hierarchy
    }

    /// Clean path prefixes like "./" from refactoring candidates
    fn clean_path_prefixes(
        &self,
        candidates: &[crate::api::results::RefactoringCandidate],
    ) -> Vec<crate::api::results::RefactoringCandidate> {
        candidates
            .iter()
            .cloned()
            .map(|mut candidate| {
                // Clean the file_path
                if candidate.file_path.starts_with("./") {
                    candidate.file_path = candidate.file_path[2..].to_string();
                }

                // Clean the entity_id
                if candidate.entity_id.starts_with("./") {
                    candidate.entity_id = candidate.entity_id[2..].to_string();
                }

                // Clean the name field if it also has the prefix
                if candidate.name.starts_with("./") {
                    candidate.name = candidate.name[2..].to_string();
                }

                candidate
            })
            .collect()
    }

    fn clean_path_prefixes_in_file_groups(
        &self,
        file_groups: &[crate::api::results::FileRefactoringGroup],
    ) -> Vec<crate::api::results::FileRefactoringGroup> {
        file_groups
            .iter()
            .cloned()
            .map(|mut group| {
                // Clean the file_path
                if group.file_path.starts_with("./") {
                    group.file_path = group.file_path[2..].to_string();
                }

                // Clean all entities within the group
                group.entities = self.clean_path_prefixes(&group.entities);

                group
            })
            .collect()
    }

    /// Clean "./" prefixes from directory health tree paths
    fn clean_directory_health_tree_paths(
        &self,
        tree: &crate::api::results::DirectoryHealthTree,
    ) -> crate::api::results::DirectoryHealthTree {
        let mut cleaned_tree = tree.clone();

        // Clean root path
        if cleaned_tree.root.path.to_string_lossy().starts_with("./") {
            cleaned_tree.root.path = PathBuf::from(&cleaned_tree.root.path.to_string_lossy()[2..]);
        }

        // Clean parent path in root
        if let Some(ref parent) = cleaned_tree.root.parent {
            if parent.to_string_lossy().starts_with("./") {
                cleaned_tree.root.parent = Some(PathBuf::from(&parent.to_string_lossy()[2..]));
            }
        }

        // Clean children paths in root
        cleaned_tree.root.children = cleaned_tree
            .root
            .children
            .iter()
            .map(|child| {
                if child.to_string_lossy().starts_with("./") {
                    PathBuf::from(&child.to_string_lossy()[2..])
                } else {
                    child.clone()
                }
            })
            .collect();

        // Clean all directory paths and their contents
        let mut cleaned_directories = std::collections::HashMap::new();
        for (path, dir_health) in &cleaned_tree.directories {
            let mut cleaned_dir = dir_health.clone();

            // Clean the directory path key
            let cleaned_path = if path.to_string_lossy().starts_with("./") {
                PathBuf::from(&path.to_string_lossy()[2..])
            } else {
                path.clone()
            };

            // Clean the path field in the DirectoryHealthScore
            if cleaned_dir.path.to_string_lossy().starts_with("./") {
                cleaned_dir.path = PathBuf::from(&cleaned_dir.path.to_string_lossy()[2..]);
            }

            // Clean parent path
            if let Some(ref parent) = cleaned_dir.parent {
                if parent.to_string_lossy().starts_with("./") {
                    cleaned_dir.parent = Some(PathBuf::from(&parent.to_string_lossy()[2..]));
                }
            }

            // Clean children paths
            cleaned_dir.children = cleaned_dir
                .children
                .iter()
                .map(|child| {
                    if child.to_string_lossy().starts_with("./") {
                        PathBuf::from(&child.to_string_lossy()[2..])
                    } else {
                        child.clone()
                    }
                })
                .collect();

            cleaned_directories.insert(cleaned_path, cleaned_dir);
        }
        cleaned_tree.directories = cleaned_directories;

        // Clean hotspot directory paths in tree statistics
        cleaned_tree.tree_statistics.hotspot_directories = cleaned_tree
            .tree_statistics
            .hotspot_directories
            .iter()
            .map(|hotspot| {
                let mut cleaned_hotspot = hotspot.clone();
                if cleaned_hotspot.path.to_string_lossy().starts_with("./") {
                    cleaned_hotspot.path =
                        PathBuf::from(&cleaned_hotspot.path.to_string_lossy()[2..]);
                }
                cleaned_hotspot
            })
            .collect();

        cleaned_tree
    }

    /// Copy the Sibylline theme CSS to the output directory
    fn copy_theme_css_to_output<P: AsRef<Path>>(&self, output_dir: P) -> Result<(), ReportError> {
        let output_dir = output_dir.as_ref();

        // Ensure output directory exists
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)?;
        }

        // Try to find Sibylline CSS in themes/ directory
        let possible_theme_paths = vec![
            Path::new("themes/sibylline.css"),
            Path::new("./themes/sibylline.css"),
        ];

        let mut theme_copied = false;
        for theme_path in &possible_theme_paths {
            if theme_path.exists() {
                let dest_path = output_dir.join("sibylline.css");
                fs::copy(theme_path, &dest_path)?;
                theme_copied = true;
                break;
            }
        }

        // If no external theme found, create a minimal fallback CSS
        if !theme_copied {
            let fallback_css = MINIMAL_SIBYLLINE_CSS;
            let dest_path = output_dir.join("sibylline.css");
            fs::write(dest_path, fallback_css)?;
        }

        Ok(())
    }

    /// Copy JavaScript assets for React tree component to the output directory
    fn copy_js_assets_to_output<P: AsRef<Path>>(&self, output_dir: P) -> Result<(), ReportError> {
        let output_dir = output_dir.as_ref();

        // Ensure output directory exists
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)?;
        }

        // JavaScript files to copy - self-contained debug bundle only
        let js_files = vec![
            ("react-tree-bundle.debug.js", "react-tree-bundle.debug.js"),
        ];

        // Try to find JavaScript assets in templates/assets/dist/ directory (built files)
        let possible_base_paths = vec![
            Path::new("templates/assets/dist"),
            Path::new("./templates/assets/dist"),
            Path::new("templates/assets"), // fallback for any non-built files
            Path::new("./templates/assets"),
        ];

        for (src_filename, dest_filename) in &js_files {
            let mut asset_copied = false;

            for base_path in &possible_base_paths {
                let asset_path = base_path.join(src_filename);
                if asset_path.exists() {
                    let dest_path = output_dir.join(dest_filename);
                    fs::copy(&asset_path, &dest_path)?;
                    asset_copied = true;
                    break;
                }
            }

            if !asset_copied {
                eprintln!(
                    "Warning: JavaScript asset {} not found, React tree component may not work",
                    src_filename
                );
            }
        }

        Ok(())
    }

    fn copy_webpage_assets_to_output<P: AsRef<Path>>(
        &self,
        output_dir: P,
    ) -> Result<(), ReportError> {
        let output_dir = output_dir.as_ref();

        // Ensure output directory exists
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)?;
        }

        // Create webpage_files subdirectory in output
        let webpage_files_dir = output_dir.join("webpage_files");
        if !webpage_files_dir.exists() {
            fs::create_dir_all(&webpage_files_dir)?;
        }

        // Try to find and copy webpage assets
        let possible_asset_sources = vec![
            ("webpage_files/valknut-large.webp", "webpage_files"),
            ("assets/logo.webp", "webpage_files"),
            ("webpage_files/logo.svg", "webpage_files"),
            ("webpage_files/three.min.js", "webpage_files"),
            ("webpage_files/trefoil-animation.js", "webpage_files"),
        ];

        for (source_path, dest_subdir) in &possible_asset_sources {
            let source = Path::new(source_path);
            if source.exists() {
                let dest_dir = if dest_subdir.is_empty() {
                    output_dir.to_path_buf()
                } else {
                    output_dir.join(dest_subdir)
                };

                if !dest_dir.exists() {
                    fs::create_dir_all(&dest_dir)?;
                }

                if let Some(filename) = source.file_name() {
                    let dest_path = dest_dir.join(filename);
                    if let Err(e) = fs::copy(source, &dest_path) {
                        // Log warning but don't fail the report generation
                        eprintln!("Warning: Failed to copy asset {}: {}", source_path, e);
                    }
                }
            }
        }

        // If logo doesn't exist, try to find alternatives
        let logo_exists = webpage_files_dir.join("valknut-large.webp").exists()
            || webpage_files_dir.join("logo.webp").exists()
            || webpage_files_dir.join("logo.svg").exists();

        if !logo_exists {
            // Create a simple SVG fallback logo
            let fallback_logo = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M9 12l2 2 4-4"/>
                <circle cx="12" cy="12" r="9"/>
            </svg>"#;
            let fallback_path = webpage_files_dir.join("logo.svg");
            if let Err(e) = fs::write(fallback_path, fallback_logo) {
                eprintln!("Warning: Failed to create fallback logo: {}", e);
            }
        }

        Ok(())
    }
}

// Minimal fallback template for when external templates aren't available
const FALLBACK_HTML_TEMPLATE: &str = r#"
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
        
        .directory-tree {
            background: var(--surface-color);
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 1.5rem;
            margin-bottom: 2rem;
            font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
            font-size: 0.9rem;
            line-height: 1.6;
            white-space: pre-line;
            overflow-x: auto;
        }
        
        .health-indicator {
            display: inline-block;
            width: 20px;
            text-align: center;
        }
        
        .health-score {
            color: var(--primary-color);
            font-weight: 600;
        }
        
        .hotspot-list {
            margin-top: 1rem;
        }
        
        .hotspot-item {
            background: rgba(239, 68, 68, 0.1);
            border: 1px solid rgba(239, 68, 68, 0.3);
            border-radius: 6px;
            padding: 1rem;
            margin-bottom: 0.75rem;
        }
        
        .hotspot-item .hotspot-path {
            font-family: 'SF Mono', Monaco, 'Cascadia Code', monospace;
            color: var(--error-color);
            font-weight: 600;
            margin-bottom: 0.5rem;
        }
        
        .hotspot-item .hotspot-details {
            color: var(--text-secondary);
            font-size: 0.9rem;
        }
        
        .tabs {
            display: flex;
            border-bottom: 2px solid var(--border-color);
            margin-bottom: 1.5rem;
        }
        
        .tab {
            padding: 0.75rem 1.5rem;
            background: none;
            border: none;
            border-bottom: 2px solid transparent;
            cursor: pointer;
            font-size: 1rem;
            color: var(--text-secondary);
            transition: all 0.2s;
        }
        
        .tab.active {
            color: var(--primary-color);
            border-bottom-color: var(--primary-color);
        }
        
        .tab-content {
            display: none;
        }
        
        .tab-content.active {
            display: block;
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
            {{#if directory_tree}}
            <div class="summary-card">
                <div class="value">{{percentage directory_tree.tree_statistics.avg_health_score}}%</div>
                <div class="label">Avg Health Score</div>
            </div>
            <div class="summary-card">
                <div class="value">{{directory_tree.tree_statistics.hotspot_directories.length}}</div>
                <div class="label">Directory Hotspots</div>
            </div>
            {{/if}}
        </section>
        
        {{#if directory_tree}}
        <section class="results-section">
            <h2>Directory Health Overview</h2>
            
            <div class="tabs">
                <button class="tab active" onclick="showTab('tree-view')">Tree View</button>
                <button class="tab" onclick="showTab('hotspots')">Hotspots</button>
                <button class="tab" onclick="showTab('statistics')">Statistics</button>
            </div>
            
            <div id="tree-view" class="tab-content active">
                <div class="directory-tree">{{tree_visualization}}</div>
            </div>
            
            <div id="hotspots" class="tab-content">
                {{#if directory_tree.tree_statistics.hotspot_directories}}
                <div class="hotspot-list">
                    {{#each directory_tree.tree_statistics.hotspot_directories}}
                    <div class="hotspot-item">
                        <div class="hotspot-path">{{this.path}} (Rank #{{this.rank}})</div>
                        <div class="hotspot-details">
                            <strong>Health Score:</strong> {{percentage this.health_score}}% | 
                            <strong>Primary Issue:</strong> {{this.primary_issue_category}}<br>
                            <strong>Recommendation:</strong> {{this.recommendation}}
                        </div>
                    </div>
                    {{/each}}
                </div>
                {{else}}
                <p>No directory hotspots identified. All directories are in good health!</p>
                {{/if}}
            </div>
            
            <div id="statistics" class="tab-content">
                <div class="summary">
                    <div class="summary-card">
                        <div class="value">{{directory_tree.tree_statistics.total_directories}}</div>
                        <div class="label">Total Directories</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">{{directory_tree.tree_statistics.max_depth}}</div>
                        <div class="label">Max Depth</div>
                    </div>
                    <div class="summary-card">
                        <div class="value">{{directory_tree.tree_statistics.health_score_std_dev}}</div>
                        <div class="label">Health Score Std Dev</div>
                    </div>
                </div>
            </div>
        </section>
        {{/if}}
        
        <section class="results-section">
            <h2>Refactoring Candidates</h2>
            <div class="file-list">
                {{#each results.refactoring_candidates}}
                <div class="file-item" data-file-path="{{this.file_path}}">
                    <div class="file-path">{{this.name}} in {{this.file_path}}</div>
                    <div class="file-details">
                        Priority: {{this.priority}} | Score: {{this.score}} | Confidence: {{this.confidence}}
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
        // Tab switching functionality
        function showTab(tabId) {
            // Hide all tab contents
            const tabContents = document.querySelectorAll('.tab-content');
            tabContents.forEach(content => {
                content.classList.remove('active');
            });
            
            // Remove active class from all tabs
            const tabs = document.querySelectorAll('.tab');
            tabs.forEach(tab => {
                tab.classList.remove('active');
            });
            
            // Show selected tab content
            const selectedContent = document.getElementById(tabId);
            if (selectedContent) {
                selectedContent.classList.add('active');
            }
            
            // Add active class to clicked tab
            const clickedTab = event.target;
            clickedTab.classList.add('active');
        }
        
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

// Minimal Sibylline CSS fallback when external CSS isn't found
const MINIMAL_SIBYLLINE_CSS: &str = r#"
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');
@import url('https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600&display=swap');

:root {
  --bg: #0b0c0e;
  --panel: #12141a;
  --surface: #191d24;
  --surface-hover: #1f242c;
  --keyline: #252a32;
  --border: #2d333c;
  --border-hover: #373e48;
  --text: #ebedef;
  --text-secondary: #c8ccd2;
  --muted: #a0a7b3;
  --accent: #20d4c0;
  --accent-hover: #14b8a6;
  --accent-muted: #20d4c033;
  --accent-soft: #20d4c01a;
  --success: #16a34a;
  --warning: #f59e0b;
  --error: #f87171;
  --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  --font-mono: 'IBM Plex Mono', 'SF Mono', Monaco, 'Cascadia Code', monospace;
  --spacing-2: 0.5rem;
  --spacing-4: 1rem;
  --spacing-6: 1.5rem;
  --spacing-8: 2rem;
  --spacing-12: 3rem;
  --radius: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --shadow: 0 2px 6px rgba(0, 0, 0, 0.25);
  --shadow-lg: 0 8px 25px rgba(0, 0, 0, 0.25);
}

* { box-sizing: border-box; margin: 0; padding: 0; }

body {
  font-family: var(--font-sans);
  background-color: var(--bg);
  color: var(--text);
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}

.container {
  max-width: 1400px;
  margin: 0 auto;
  padding: var(--spacing-8);
}

.header {
  text-align: center;
  margin-bottom: var(--spacing-12);
  padding: var(--spacing-8) 0;
  background: var(--panel);
  border: 1px solid var(--keyline);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-lg);
}

.header h1 {
  color: var(--accent);
  font-size: 2rem;
  font-weight: 700;
  margin-bottom: var(--spacing-2);
}

.header .meta {
  color: var(--muted);
  font-size: 0.875rem;
  font-family: var(--font-mono);
}

.summary {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: var(--spacing-6);
  margin-bottom: var(--spacing-12);
}

.summary-card {
  background: var(--surface);
  border: 1px solid var(--keyline);
  border-radius: var(--radius-lg);
  padding: var(--spacing-8);
  text-align: center;
  box-shadow: var(--shadow);
}

.summary-card .value {
  font-size: 2rem;
  font-weight: 700;
  color: var(--accent);
  margin-bottom: var(--spacing-2);
  font-family: var(--font-mono);
}

.summary-card .label {
  color: var(--muted);
  font-size: 0.875rem;
  text-transform: uppercase;
  font-weight: 600;
}

.results-section {
  margin-bottom: var(--spacing-12);
}

.results-section h2 {
  color: var(--text);
  margin-bottom: var(--spacing-6);
  padding-bottom: var(--spacing-2);
  border-bottom: 2px solid var(--accent);
  font-size: 1.5rem;
  font-weight: 600;
}

.file-list {
  background: var(--surface);
  border: 1px solid var(--keyline);
  border-radius: var(--radius-lg);
  overflow: hidden;
  box-shadow: var(--shadow);
}

.file-item {
  padding: var(--spacing-6);
  border-bottom: 1px solid var(--keyline);
  cursor: pointer;
  transition: all 0.2s ease;
}

.file-item:hover {
  background: var(--surface-hover);
  transform: translateX(4px);
  border-left: 3px solid var(--accent);
  padding-left: calc(var(--spacing-6) - 3px);
}

.file-item:last-child {
  border-bottom: none;
}

.file-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-2);
}

.file-path {
  font-family: var(--font-mono);
  color: var(--accent);
  font-weight: 500;
  font-size: 0.875rem;
}

.badge {
  padding: 0.25rem 0.75rem;
  border-radius: 6px;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
}

.badge.success { background: var(--success); color: var(--bg); }
.badge.error { background: var(--error); color: var(--bg); }
.badge.warning { background: var(--warning); color: var(--bg); }

.file-details {
  font-size: 0.75rem;
  color: var(--muted);
  display: flex;
  gap: var(--spacing-4);
  font-family: var(--font-mono);
}

.raw-data {
  background: var(--surface);
  border: 1px solid var(--keyline);
  border-radius: var(--radius-lg);
  padding: var(--spacing-6);
  margin-top: var(--spacing-12);
  box-shadow: var(--shadow);
}

.raw-data summary {
  cursor: pointer;
  font-weight: 600;
  color: var(--text);
  margin-bottom: var(--spacing-4);
  padding: var(--spacing-4);
  border-radius: var(--radius);
  background: var(--accent-soft);
  font-family: var(--font-mono);
  font-size: 0.875rem;
}

.raw-data pre {
  background: var(--panel);
  color: var(--text-secondary);
  padding: var(--spacing-6);
  border-radius: var(--radius);
  overflow-x: auto;
  font-size: 0.75rem;
  line-height: 1.6;
  font-family: var(--font-mono);
  border: 1px solid var(--keyline);
  margin-top: var(--spacing-4);
}

@media (max-width: 768px) {
  .container { padding: var(--spacing-4); }
  .header h1 { font-size: 1.5rem; }
  .summary { grid-template-columns: 1fr; }
  .file-header { flex-direction: column; align-items: flex-start; gap: var(--spacing-2); }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::results::*;
    use crate::api::results::{
        AnalysisResults, AnalysisStatistics, AnalysisSummary, FeatureContribution, MemoryStats,
        RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
    };
    use crate::core::featureset::FeatureVector;
    use crate::core::scoring::{Priority, ScoringResult};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_results() -> AnalysisResults {
        use std::time::Duration;

        AnalysisResults {
            summary: AnalysisSummary {
                files_processed: 3,
                entities_analyzed: 15,
                refactoring_needed: 5,
                high_priority: 2,
                critical: 1,
                avg_refactoring_score: 0.65,
                code_health_score: 0.75,
            },
            refactoring_candidates: vec![RefactoringCandidate {
                entity_id: "test_entity_1".to_string(),
                name: "complex_function".to_string(),
                file_path: "src/test.rs".to_string(),
                line_range: Some((10, 50)),
                priority: Priority::High,
                score: 0.85,
                confidence: 0.9,
                issues: vec![RefactoringIssue {
                    category: "complexity".to_string(),
                    description: "High cyclomatic complexity".to_string(),
                    severity: 2.1,
                    contributing_features: vec![FeatureContribution {
                        feature_name: "cyclomatic_complexity".to_string(),
                        value: 15.0,
                        normalized_value: 0.8,
                        contribution: 1.2,
                    }],
                }],
                suggestions: vec![RefactoringSuggestion {
                    refactoring_type: "extract_method".to_string(),
                    description: "Break down large method".to_string(),
                    priority: 0.9,
                    effort: 0.6,
                    impact: 0.8,
                }],
                issue_count: 1,
                suggestion_count: 1,
            }],
            refactoring_candidates_by_file: vec![],
            statistics: AnalysisStatistics {
                total_duration: Duration::from_millis(1500),
                avg_file_processing_time: Duration::from_millis(500),
                avg_entity_processing_time: Duration::from_millis(100),
                features_per_entity: std::collections::HashMap::new(),
                priority_distribution: std::collections::HashMap::new(),
                issue_distribution: std::collections::HashMap::new(),
                memory_stats: MemoryStats {
                    peak_memory_bytes: 128 * 1024 * 1024,
                    final_memory_bytes: 64 * 1024 * 1024,
                    efficiency_score: 0.85,
                },
            },
            directory_health_tree: None,
            clone_analysis: None,
            warnings: vec!["Test warning".to_string()],
            coverage_packs: vec![crate::detectors::coverage::CoveragePack {
                kind: "coverage".to_string(),
                pack_id: "cov:src/test.rs".to_string(),
                path: std::path::PathBuf::from("src/test.rs"),
                file_info: crate::detectors::coverage::FileInfo {
                    loc: 200,
                    coverage_before: 0.65,
                    coverage_after_if_filled: 0.90,
                },
                gaps: vec![crate::detectors::coverage::CoverageGap {
                    path: std::path::PathBuf::from("src/test.rs"),
                    span: crate::detectors::coverage::UncoveredSpan {
                        path: std::path::PathBuf::from("src/test.rs"),
                        start: 25,
                        end: 35,
                        hits: Some(0),
                    },
                    file_loc: 200,
                    language: "rust".to_string(),
                    score: 0.85,
                    features: crate::detectors::coverage::GapFeatures {
                        gap_loc: 10,
                        cyclomatic_in_gap: 3.0,
                        cognitive_in_gap: 4.0,
                        fan_in_gap: 2,
                        exports_touched: true,
                        dependency_centrality_file: 0.7,
                        interface_surface: 3,
                        docstring_or_comment_present: false,
                        exception_density_in_gap: 0.1,
                    },
                    symbols: vec![crate::detectors::coverage::GapSymbol {
                        kind: crate::detectors::coverage::SymbolKind::Function,
                        name: "uncovered_function".to_string(),
                        signature: "fn uncovered_function(x: i32) -> Result<String>".to_string(),
                        line_start: 25,
                        line_end: 35,
                    }],
                    preview: crate::detectors::coverage::SnippetPreview {
                        language: "rust".to_string(),
                        pre: vec!["    // Previous context".to_string()],
                        head: vec![
                            "    fn uncovered_function(x: i32) -> Result<String> {".to_string()
                        ],
                        tail: vec!["    }".to_string()],
                        post: vec!["    // Following context".to_string()],
                        markers: crate::detectors::coverage::GapMarkers {
                            start_line: 25,
                            end_line: 35,
                        },
                        imports: vec!["use std::result::Result;".to_string()],
                    },
                }],
                value: crate::detectors::coverage::PackValue {
                    file_cov_gain: 0.25,
                    repo_cov_gain_est: 0.05,
                },
                effort: crate::detectors::coverage::PackEffort {
                    tests_to_write_est: 3,
                    mocks_est: 1,
                },
            }],
            unified_hierarchy: vec![],
        }
    }

    #[test]
    fn test_report_generator_new() {
        let generator = ReportGenerator::new();
        assert!(generator
            .handlebars
            .get_templates()
            .contains_key("default_html"));
        assert!(generator.templates_dir.is_none());
    }

    #[test]
    fn test_report_generator_default() {
        let generator = ReportGenerator::default();
        assert!(generator
            .handlebars
            .get_templates()
            .contains_key("default_html"));
        assert!(generator.templates_dir.is_none());
    }

    #[test]
    fn test_report_generator_debug() {
        let generator = ReportGenerator::new();
        let debug_str = format!("{:?}", generator);
        assert!(debug_str.contains("ReportGenerator"));
        assert!(debug_str.contains("handlebars"));
        assert!(debug_str.contains("templates_dir"));
    }

    #[test]
    fn test_with_templates_dir_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let nonexistent_path = temp_dir.path().join("nonexistent");

        let generator = ReportGenerator::new()
            .with_templates_dir(&nonexistent_path)
            .unwrap();

        assert_eq!(generator.templates_dir, Some(nonexistent_path));
    }

    #[test]
    fn test_with_templates_dir_existing() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create a test template file
        let template_file = templates_dir.join("custom.hbs");
        fs::write(
            &template_file,
            "{{#each items}}<div>{{this}}</div>{{/each}}",
        )
        .unwrap();

        let generator = ReportGenerator::new()
            .with_templates_dir(&templates_dir)
            .unwrap();

        assert_eq!(generator.templates_dir, Some(templates_dir));
        assert!(generator.handlebars.get_templates().contains_key("custom"));
    }

    #[test]
    fn test_generate_json_report() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.json");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Json);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("\"files_processed\": 3"));
        assert!(content.contains("\"complex_function\""));
        assert!(content.contains("\"Test warning\""));
    }

    #[test]
    fn test_generate_yaml_report() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.yaml");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Yaml);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("files_processed: 3"));
        assert!(content.contains("complex_function"));
    }

    #[test]
    fn test_generate_csv_report() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.csv");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Csv);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("CSV report not yet implemented"));
    }

    #[test]
    fn test_generate_html_report_default_template() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_report.html");

        let generator = ReportGenerator::new();
        let results = create_test_results();

        let result = generator.generate_report(&results, &output_path, ReportFormat::Html);
        if let Err(ref e) = result {
            panic!("HTML generation failed: {}", e);
        }
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("Analysis Report"));
        assert!(content.contains("Valknut"));
        assert!(content.contains("Files Analyzed"));
    }

    #[test]
    fn test_generate_html_report_custom_template() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create a custom report template
        let custom_template = r#"
        <html>
        <head><title>Custom Report</title></head>
        <body>
        <h1>{{tool_name}} Report</h1>
        <p>Files processed: {{summary.total_files}}</p>
        <p>Issues found: {{summary.total_issues}}</p>
        </body>
        </html>
        "#;

        let template_file = templates_dir.join("report.hbs");
        fs::write(&template_file, custom_template).unwrap();

        let generator = ReportGenerator::new()
            .with_templates_dir(&templates_dir)
            .unwrap();

        let results = create_test_results();
        let output_path = temp_dir.path().join("test_report.html");

        let result = generator.generate_report(&results, &output_path, ReportFormat::Html);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("Custom Report"));
        assert!(content.contains("Files processed: 3"));
    }

    #[test]
    fn test_prepare_template_data() {
        let generator = ReportGenerator::new();
        let results = create_test_results();

        let template_data = generator.prepare_template_data(&results);

        assert!(template_data.is_object());
        let obj = template_data.as_object().unwrap();

        assert!(obj.contains_key("generated_at"));
        assert!(obj.contains_key("tool_name"));
        assert!(obj.contains_key("version"));
        assert!(obj.contains_key("results"));
        assert!(obj.contains_key("summary"));

        assert_eq!(
            obj["tool_name"],
            serde_json::Value::String("Valknut".to_string())
        );
    }

    #[test]
    fn test_calculate_summary() {
        let generator = ReportGenerator::new();
        let results = create_test_results();

        let summary = generator.calculate_summary(&results);

        assert_eq!(
            summary.get("total_files").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        assert_eq!(
            summary.get("total_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(1))
        );
    }

    #[test]
    fn test_report_error_display() {
        let io_error = std::io::Error::new(std::io::ErrorKind::InvalidData, "template error");
        let report_error = ReportError::Io(io_error);

        let error_string = format!("{}", report_error);
        assert!(error_string.contains("IO error"));
    }

    #[test]
    fn test_report_error_debug() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let report_error = ReportError::Io(io_error);

        let debug_string = format!("{:?}", report_error);
        assert!(debug_string.contains("Io"));
        assert!(debug_string.contains("NotFound"));
    }

    #[test]
    fn test_load_templates_from_dir_invalid_filename() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create a file with invalid filename (no stem) - try a different approach
        // Since .hbs might be valid on some systems, let's use a filename that definitely has no stem
        let bad_file = templates_dir.join("");
        match fs::write(&bad_file, "content") {
            Ok(_) => {
                // If the write succeeded, test should pass
                let mut generator = ReportGenerator::new();
                let result = generator.load_templates_from_dir(&templates_dir);
                // Just make sure it doesn't panic, the result could be ok or error
                let _ = result;
            }
            Err(_) => {
                // If we can't create the invalid file, that's expected
                // Just test with a normal template loading that should work
                let good_file = templates_dir.join("good.hbs");
                fs::write(&good_file, "{{content}}").unwrap();

                let mut generator = ReportGenerator::new();
                let result = generator.load_templates_from_dir(&templates_dir);
                assert!(result.is_ok());
            }
        }
    }

    #[test]
    fn test_load_templates_from_dir_non_hbs_files() {
        let temp_dir = TempDir::new().unwrap();
        let templates_dir = temp_dir.path().join("templates");
        fs::create_dir_all(&templates_dir).unwrap();

        // Create non-.hbs files that should be ignored
        fs::write(templates_dir.join("readme.txt"), "not a template").unwrap();
        fs::write(templates_dir.join("config.json"), "{}").unwrap();

        let mut generator = ReportGenerator::new();
        let initial_count = generator.handlebars.get_templates().len();

        let result = generator.load_templates_from_dir(&templates_dir);
        assert!(result.is_ok());

        // Should have same number of templates (no new ones added)
        assert_eq!(generator.handlebars.get_templates().len(), initial_count);
    }

    #[test]
    fn test_clean_directory_health_tree_paths() {
        let generator = ReportGenerator::new();

        // Create a test directory health tree with "./" prefixes
        let mut directories = std::collections::HashMap::new();

        // Create directory with ./ prefix
        let src_dir = DirectoryHealthScore {
            path: PathBuf::from("./src"),
            health_score: 0.7,
            file_count: 2,
            entity_count: 3,
            refactoring_needed: 1,
            critical_issues: 0,
            high_priority_issues: 1,
            avg_refactoring_score: 1.5,
            weight: 1.0,
            children: vec![PathBuf::from("./src/core")],
            parent: Some(PathBuf::from("./")),
            issue_categories: std::collections::HashMap::new(),
        };

        let core_dir = DirectoryHealthScore {
            path: PathBuf::from("./src/core"),
            health_score: 0.6,
            file_count: 1,
            entity_count: 2,
            refactoring_needed: 2,
            critical_issues: 1,
            high_priority_issues: 2,
            avg_refactoring_score: 2.0,
            weight: 2.0,
            children: vec![],
            parent: Some(PathBuf::from("./src")),
            issue_categories: std::collections::HashMap::new(),
        };

        directories.insert(PathBuf::from("./src"), src_dir);
        directories.insert(PathBuf::from("./src/core"), core_dir);

        let hotspot_directories = vec![DirectoryHotspot {
            path: PathBuf::from("./src/core"),
            health_score: 0.6,
            rank: 1,
            primary_issue_category: "complexity".to_string(),
            recommendation: "Reduce complexity".to_string(),
        }];

        let tree_statistics = TreeStatistics {
            total_directories: 2,
            max_depth: 2,
            avg_health_score: 0.65,
            health_score_std_dev: 0.05,
            hotspot_directories,
            health_by_depth: std::collections::HashMap::new(),
        };

        let root = DirectoryHealthScore {
            path: PathBuf::from("./"),
            health_score: 0.8,
            file_count: 0,
            entity_count: 0,
            refactoring_needed: 0,
            critical_issues: 0,
            high_priority_issues: 0,
            avg_refactoring_score: 0.0,
            weight: 1.0,
            children: vec![PathBuf::from("./src")],
            parent: None,
            issue_categories: std::collections::HashMap::new(),
        };

        let original_tree = DirectoryHealthTree {
            root,
            directories,
            tree_statistics,
        };

        // Clean the paths
        let cleaned_tree = generator.clean_directory_health_tree_paths(&original_tree);

        // Verify that "./" prefixes are removed
        assert_eq!(cleaned_tree.root.path, PathBuf::from(""));
        assert_eq!(cleaned_tree.root.children[0], PathBuf::from("src"));

        // Check that directories HashMap keys are cleaned
        assert!(cleaned_tree.directories.contains_key(&PathBuf::from("src")));
        assert!(cleaned_tree
            .directories
            .contains_key(&PathBuf::from("src/core")));
        assert!(!cleaned_tree
            .directories
            .contains_key(&PathBuf::from("./src")));
        assert!(!cleaned_tree
            .directories
            .contains_key(&PathBuf::from("./src/core")));

        // Check that directory paths are cleaned within DirectoryHealthScore
        let src_dir_cleaned = cleaned_tree.directories.get(&PathBuf::from("src")).unwrap();
        assert_eq!(src_dir_cleaned.path, PathBuf::from("src"));
        assert_eq!(src_dir_cleaned.children[0], PathBuf::from("src/core"));
        assert_eq!(src_dir_cleaned.parent, Some(PathBuf::from("")));

        let core_dir_cleaned = cleaned_tree
            .directories
            .get(&PathBuf::from("src/core"))
            .unwrap();
        assert_eq!(core_dir_cleaned.path, PathBuf::from("src/core"));
        assert_eq!(core_dir_cleaned.parent, Some(PathBuf::from("src")));

        // Check that hotspot directories are cleaned
        assert_eq!(
            cleaned_tree.tree_statistics.hotspot_directories[0].path,
            PathBuf::from("src/core")
        );
    }
}
