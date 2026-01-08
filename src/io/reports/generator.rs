//! Report generation with template engine support.

use crate::api::config_types::AnalysisConfig;
use crate::core::config::ReportFormat;
use crate::core::pipeline::{
    AnalysisResults, CodeDictionary, DepthHealthStats, DirectoryHealthScore, DirectoryHealthTree,
    DirectoryHotspot, DirectoryIssueSummary, FileRefactoringGroup, NormalizedAnalysisResults,
    NormalizedEntity, RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
    TreeStatistics,
};
use crate::core::scoring::Priority;
use chrono::Utc;
use handlebars::{Handlebars, Renderable};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use super::assets::{
    copy_js_assets_to_output, copy_theme_css_to_output, copy_webpage_assets_to_output,
};
use super::error::ReportError;
use super::helpers::{register_helpers, safe_json_value};
use super::hierarchy::create_file_groups_from_candidates;
use super::templates::{
    detect_templates_dir, load_templates_from_dir, register_fallback_template, CSV_TEMPLATE_NAME,
    FALLBACK_TEMPLATE_NAME, MARKDOWN_TEMPLATE_NAME, SONAR_TEMPLATE_NAME,
};

#[derive(Debug)]
pub struct ReportGenerator {
    handlebars: Handlebars<'static>,
    templates_dir: Option<PathBuf>,
    analysis_config: Option<AnalysisConfig>,
}

impl Default for ReportGenerator {
    fn default() -> Self {
        let mut handlebars = Handlebars::new();
        register_helpers(&mut handlebars);
        register_fallback_template(&mut handlebars);

        let mut generator = Self {
            handlebars,
            templates_dir: None,
            analysis_config: None,
        };

        if let Some(templates_dir) = detect_templates_dir() {
            if let Err(err) = load_templates_from_dir(&mut generator.handlebars, &templates_dir) {
                eprintln!("Failed to load external templates: {}", err);
            } else {
                generator.templates_dir = Some(templates_dir);
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
            load_templates_from_dir(&mut self.handlebars, &templates_dir)?;
        }

        self.templates_dir = Some(templates_dir);
        Ok(self)
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

    pub fn generate_markdown_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        self.render_template_to_path(MARKDOWN_TEMPLATE_NAME, results, output_path)
    }

    pub fn generate_csv_table<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        self.render_template_to_path(CSV_TEMPLATE_NAME, results, output_path)
    }

    pub fn generate_sonar_report<P: AsRef<Path>>(
        &self,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        self.render_template_to_path(SONAR_TEMPLATE_NAME, results, output_path)
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

        // Copy webpage assets (logo, animation files) to output directory
        // Note: CSS and JavaScript are now inlined in templates
        copy_webpage_assets_to_output(output_dir)?;

        let template_data = self.prepare_template_data_with_oracle(results, oracle_response);

        // Prefer external template over fallback
        let template_name = if self.handlebars.get_templates().contains_key("report") {
            "report"
        } else {
            FALLBACK_TEMPLATE_NAME
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
        // Stream directly to file to avoid building large string in memory
        let file = File::create(output_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &combined_result)?;
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
        results: &AnalysisResults,
        oracle_response: &Option<crate::oracle::RefactoringOracleResponse>,
        output_path: P,
    ) -> Result<(), ReportError> {
        let data = self.prepare_template_data_with_oracle(results, oracle_response);
        let rendered = self.render_template(CSV_TEMPLATE_NAME, &data)?;
        fs::write(output_path, rendered)?;
        Ok(())
    }

    fn render_template_to_path<P: AsRef<Path>>(
        &self,
        template_name: &str,
        results: &AnalysisResults,
        output_path: P,
    ) -> Result<(), ReportError> {
        let data = self.prepare_template_data(results);
        let rendered = self.render_template(template_name, &data)?;
        fs::write(output_path, rendered)?;
        Ok(())
    }

    fn render_template(&self, template_name: &str, data: &Value) -> Result<String, ReportError> {
        let rendered = self.handlebars.render(template_name, data)?;
        Ok(rendered)
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
        data.insert("tool_name", safe_json_value("Valknut"));
        data.insert(
            "version",
            serde_json::to_value(env!("CARGO_PKG_VERSION")).unwrap(),
        );

        // Add theme CSS reference - Sibylline by default
        data.insert("theme_css_url", safe_json_value("sibylline.css"));

        // Add animation config
        let enable_animation = true; // Always enable animation for now
        data.insert("enable_animation", safe_json_value(enable_animation));

        // Add Oracle refactoring plan at the TOP for user requirement
        if let Some(oracle) = oracle_response {
            data.insert("oracle_refactoring_plan", safe_json_value(oracle));
            data.insert("has_oracle_data", safe_json_value(true));
        } else {
            data.insert("has_oracle_data", safe_json_value(false));
        }

        // Add analysis results
        data.insert("results", safe_json_value(results));

        // Paths are stored as relative at creation time, so no cleaning needed
        let candidates = &results.refactoring_candidates;
        data.insert("refactoring_candidates", safe_json_value(candidates));

        let candidates_by_file = create_file_groups_from_candidates(candidates);
        data.insert(
            "refactoring_candidates_by_file",
            safe_json_value(&candidates_by_file),
        );
        data.insert(
            "file_count",
            serde_json::to_value(candidates_by_file.len()).unwrap(),
        );

        // Add summary statistics
        if let Ok(summary) = serde_json::to_value(self.calculate_summary(results)) {
            data.insert("summary", summary);
        }

        // Add code_dictionary for suggestion lookups
        if let Ok(dict_value) = serde_json::to_value(&results.code_dictionary) {
            data.insert("code_dictionary", dict_value.clone());
            data.insert("codeDictionary", dict_value);
        }

        // Build directory health tree from file health scores (covers all files, not just candidates)
        let directory_tree = if !results.file_health.is_empty() {
            Some(DirectoryHealthTree::from_file_health(&results.file_health))
        } else if !candidates.is_empty() {
            Some(DirectoryHealthTree::from_candidates(candidates))
        } else {
            None
        };

        // Build full tree payload with directory health tree
        data.insert(
            "tree_payload",
            self.build_tree_payload(results, &candidates_by_file, &directory_tree),
        );

        // Add documentation data for treemap doc health coloring
        if let Some(doc) = &results.documentation {
            data.insert("documentation", safe_json_value(doc));
        }

        // Add cohesion analysis data for semantic alignment tab
        data.insert("cohesion", safe_json_value(&results.passes.cohesion));

        // Add precomputed health scores (same formula as project health)
        data.insert("directory_health", safe_json_value(&results.directory_health));
        data.insert("directoryHealth", safe_json_value(&results.directory_health));
        data.insert("file_health", safe_json_value(&results.file_health));
        data.insert("fileHealth", safe_json_value(&results.file_health));
        data.insert("entity_health", safe_json_value(&results.entity_health));
        data.insert("entityHealth", safe_json_value(&results.entity_health));

        serde_json::to_value(data).unwrap_or_else(|_| serde_json::Value::Null)
    }

    fn calculate_summary(&self, results: &AnalysisResults) -> HashMap<String, Value> {
        let mut summary = HashMap::new();

        summary.insert(
            "files_processed".to_string(),
            serde_json::to_value(results.files_analyzed()).unwrap(),
        );
        summary.insert(
            "entities_analyzed".to_string(),
            safe_json_value(results.summary.entities_analyzed),
        );
        summary.insert(
            "refactoring_needed".to_string(),
            serde_json::to_value(results.refactoring_candidates.len()).unwrap(),
        );
        summary.insert(
            "code_health_score".to_string(),
            safe_json_value(results.summary.code_health_score),
        );
        summary.insert(
            "total_files".to_string(),
            serde_json::to_value(results.files_analyzed()).unwrap(),
        );
        summary.insert(
            "total_issues".to_string(),
            safe_json_value(
                results
                    .summary
                    .total_issues
                    .max(results.refactoring_candidates.len()),
            ),
        );
        summary.insert(
            "high_issues".to_string(),
            safe_json_value(results.summary.high_priority_issues),
        );
        summary.insert(
            "critical_issues".to_string(),
            safe_json_value(results.summary.critical_issues),
        );
        summary.insert(
            "languages".to_string(),
            safe_json_value(&results.summary.languages),
        );
        summary.insert(
            "timestamp".to_string(),
            safe_json_value(Utc::now().to_rfc3339()),
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

    fn build_tree_payload(
        &self,
        results: &AnalysisResults,
        cleaned_groups: &[FileRefactoringGroup],
        directory_tree: &Option<DirectoryHealthTree>,
    ) -> Value {
        let mut payload = serde_json::Map::new();

        if let Ok(dict_value) = serde_json::to_value(&results.code_dictionary) {
            payload.insert("code_dictionary".into(), dict_value.clone());
            payload.insert("codeDictionary".into(), dict_value);
        }

        if let Ok(coverage_value) = serde_json::to_value(&results.coverage_packs) {
            payload.insert("coverage_packs".into(), coverage_value.clone());
            payload.insert("coveragePacks".into(), coverage_value);
        }

        if let Some(normalized) = results.normalized.as_ref() {
            if let Ok(norm_value) = serde_json::to_value(normalized) {
                payload.insert("normalized_results".into(), norm_value.clone());
                payload.insert("normalizedResults".into(), norm_value);
            }

            if let Some(mut tree) = directory_tree.clone() {
                // Apply precomputed health scores for consistency with overall project health
                tree.apply_health_overlays(&results.directory_health);
                if let Some(doc) = &results.documentation {
                    tree.apply_doc_overlays(&doc.directory_doc_health, &doc.directory_doc_issues);
                }
                if let Ok(tree_value) = serde_json::to_value(&tree) {
                    payload.insert("directory_health_tree".into(), tree_value.clone());
                    payload.insert("directoryHealthTree".into(), tree_value);
                }
            }
        } else {
            if let Ok(groups_value) = serde_json::to_value(cleaned_groups) {
                payload.insert("refactoringCandidatesByFile".into(), groups_value);
            }

            if let Some(mut tree) = directory_tree.clone() {
                // Apply precomputed health scores for consistency with overall project health
                tree.apply_health_overlays(&results.directory_health);
                if let Some(doc) = &results.documentation {
                    tree.apply_doc_overlays(&doc.directory_doc_health, &doc.directory_doc_issues);
                }
                if let Ok(tree_value) = serde_json::to_value(&tree) {
                    payload.insert("directory_health_tree".into(), tree_value.clone());
                    payload.insert("directoryHealthTree".into(), tree_value);
                }
            }
        }

        // Add documentation data for treemap doc health coloring
        if let Some(doc) = &results.documentation {
            if let Ok(doc_value) = serde_json::to_value(doc) {
                payload.insert("documentation".into(), doc_value);
            }
        }

        // Add precomputed file health scores (same formula as project health)
        if let Ok(file_health_value) = serde_json::to_value(&results.file_health) {
            payload.insert("file_health".into(), file_health_value.clone());
            payload.insert("fileHealth".into(), file_health_value);
        }

        // Add precomputed directory health scores
        if let Ok(dir_health_value) = serde_json::to_value(&results.directory_health) {
            payload.insert("directory_health".into(), dir_health_value.clone());
            payload.insert("directoryHealth".into(), dir_health_value);
        }

        Value::Object(payload)
    }

    fn normalized_entities_to_candidates(
        &self,
        normalized: &NormalizedAnalysisResults,
        dictionary: &CodeDictionary,
    ) -> Vec<RefactoringCandidate> {
        normalized
            .entities
            .iter()
            .map(|entity| self.normalized_entity_to_candidate(entity, dictionary))
            .collect()
    }

    fn normalized_entity_to_candidate(
        &self,
        entity: &NormalizedEntity,
        dictionary: &CodeDictionary,
    ) -> RefactoringCandidate {
        let issues = entity
            .issues
            .iter()
            .map(|issue| {
                let category = dictionary
                    .issues
                    .get(&issue.code)
                    .and_then(|def| def.category.clone())
                    .unwrap_or_else(|| issue.category.clone());
                RefactoringIssue {
                    code: issue.code.clone(),
                    category,
                    severity: issue.severity,
                    contributing_features: Vec::new(),
                }
            })
            .collect::<Vec<_>>();

        let suggestions = entity
            .suggestions
            .iter()
            .map(|suggestion| {
                let refactoring_type = dictionary
                    .suggestions
                    .get(&suggestion.code)
                    .map(|def| def.title.clone())
                    .unwrap_or_else(|| suggestion.refactoring_type.clone());
                RefactoringSuggestion {
                    refactoring_type,
                    code: suggestion.code.clone(),
                    priority: suggestion.priority,
                    effort: suggestion.effort,
                    impact: suggestion.impact,
                }
            })
            .collect::<Vec<_>>();

        let file_path = entity
            .file_path
            .as_ref()
            .or(entity.file.as_ref())
            .cloned()
            .unwrap_or_default();

        let name = if entity.name.is_empty() {
            self.derive_entity_name(entity)
        } else {
            entity.name.clone()
        };

        RefactoringCandidate {
            entity_id: entity.id.clone(),
            name,
            file_path,
            line_range: None,
            priority: entity.priority,
            score: entity.score,
            confidence: 1.0,
            issues,
            suggestions,
            issue_count: entity.issues.len(),
            suggestion_count: entity.suggestions.len(),
            coverage_percentage: None,
        }
    }
    fn derive_entity_name(&self, entity: &NormalizedEntity) -> String {
        entity
            .id
            .rsplit(':')
            .next()
            .unwrap_or_else(|| entity.id.as_str())
            .to_string()
    }

    fn derive_directory_tree_from_normalized(
        &self,
        normalized: &NormalizedAnalysisResults,
        dictionary: &CodeDictionary,
    ) -> Option<DirectoryHealthTree> {
        let candidates = self.normalized_entities_to_candidates(normalized, dictionary);
        if candidates.is_empty() {
            None
        } else {
            Some(DirectoryHealthTree::from_candidates(&candidates))
        }
    }

}


#[cfg(test)]
#[path = "generator_tests.rs"]
mod tests;
