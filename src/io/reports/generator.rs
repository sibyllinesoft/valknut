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
use std::fs;
use std::path::{Path, PathBuf};

use super::assets::{
    copy_js_assets_to_output, copy_theme_css_to_output, copy_webpage_assets_to_output,
};
use super::error::ReportError;
use super::helpers::{register_helpers, safe_json_value};
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

        let source_candidates = results.refactoring_candidates.clone();

        let cleaned_candidates = self.clean_path_prefixes(&source_candidates);
        data.insert(
            "refactoring_candidates",
            safe_json_value(&cleaned_candidates),
        );

        let base_groups = self.create_file_groups_from_candidates(&source_candidates);
        let cleaned_candidates_by_file = self.clean_path_prefixes_in_file_groups(&base_groups);
        data.insert(
            "refactoring_candidates_by_file",
            safe_json_value(&cleaned_candidates_by_file),
        );
        data.insert(
            "file_count",
            serde_json::to_value(cleaned_candidates_by_file.len()).unwrap(),
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

        // Minimal tree payload to satisfy template consumers (legacy fields removed)
        data.insert(
            "tree_payload",
            serde_json::json!({
                "refactoring_candidates": cleaned_candidates,
                "refactoring_candidates_by_file": cleaned_candidates_by_file,
                "code_dictionary": &results.code_dictionary,
                "codeDictionary": &results.code_dictionary,
            }),
        );

        // Add documentation data for treemap doc health coloring
        if let Some(doc) = &results.documentation {
            data.insert("documentation", safe_json_value(doc));
        }

        // Add cohesion analysis data for semantic alignment tab
        data.insert("cohesion", safe_json_value(&results.passes.cohesion));

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
                if let Some(doc) = &results.documentation {
                    tree.apply_doc_overlays(&doc.directory_doc_health, &doc.directory_doc_issues);
                }
                if let Ok(tree_value) = serde_json::to_value(&tree) {
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

    /// Build a unified hierarchy combining directory health with refactoring candidates
    fn build_unified_hierarchy(
        &self,
        tree: &DirectoryHealthTree,
        file_groups: &[FileRefactoringGroup],
    ) -> Vec<serde_json::Value> {
        use std::collections::{BTreeMap, HashMap};
        use std::path::Path;

        // Map directories for lookup
        let mut dir_map: HashMap<String, &DirectoryHealthScore> = HashMap::new();
        for (path_buf, dir) in &tree.directories {
            dir_map.insert(path_buf.to_string_lossy().to_string(), dir);
        }

        // Group files by directory
        let mut files_by_dir: BTreeMap<String, Vec<&FileRefactoringGroup>> = BTreeMap::new();
        for group in file_groups {
            let dir = Path::new(&group.file_path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string());
            files_by_dir.entry(dir).or_default().push(group);
        }

        // Recursively build nodes
        fn build_dir_node(
            path: &str,
            dir: &DirectoryHealthScore,
            dir_map: &HashMap<String, &DirectoryHealthScore>,
            files_by_dir: &BTreeMap<String, Vec<&FileRefactoringGroup>>,
        ) -> serde_json::Value {
            let mut children = Vec::new();

            // Child directories
            for (child_path, child_dir) in dir_map.iter() {
                if let Some(parent) = &child_dir.parent {
                    if parent.to_string_lossy() == path {
                        children.push(build_dir_node(child_path, child_dir, dir_map, files_by_dir));
                    }
                }
            }

            // File children
            if let Some(files) = files_by_dir.get(path) {
                for file_group in files {
                    let total_issues: usize =
                        file_group.entities.iter().map(|e| e.issues.len()).sum();
                    let entity_count = file_group.entities.len().max(1);
                    let file_health =
                        (1.0 - (total_issues as f64 / entity_count as f64)).clamp(0.0, 1.0);

                    let entities: Vec<serde_json::Value> = file_group
                        .entities
                        .iter()
                        .map(|entity| {
                            let mut v = serde_json::to_value(entity).unwrap_or_default();
                            v["type"] = serde_json::Value::String("entity".to_string());
                            // Ensure a stable id for tree rendering
                            if v.get("id").is_none() {
                                let id = if !entity.entity_id.is_empty() {
                                    entity.entity_id.clone()
                                } else {
                                    entity.name.clone()
                                };
                                v["id"] = serde_json::Value::String(format!(
                                    "entity_{}",
                                    id.replace('/', "_").replace(':', "_")
                                ));
                            }
                            v
                        })
                        .collect();

                    children.push(serde_json::json!({
                        "id": format!("file_{}", file_group.file_path.replace('/', "_")),
                        "type": "file",
                        "path": file_group.file_path,
                        "name": file_group.file_name,
                        "entity_count": file_group.entity_count,
                        "avg_score": ((file_group.avg_score * 10.0).round() / 10.0),
                        "priority": file_group.highest_priority,
                        "health_score": file_health,
                        "total_issues": total_issues,
                        "children": entities
                    }));
                }
            }

            children.sort_by(|a, b| {
                let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
                name_a.cmp(name_b)
            });

            let display_name = Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string());

            serde_json::json!({
                "id": format!("directory_{}", path.replace('/', "_")),
                        "type": "folder",
                "path": path,
                "name": display_name,
                "health_score": dir.health_score,
                "entity_count": dir.entity_count,
                "file_count": dir.file_count,
                "refactoring_needed": dir.refactoring_needed,
                "children": children
            })
        }

        let mut roots = Vec::new();
        for (path, dir) in dir_map.iter() {
            let is_root = dir
                .parent
                .as_ref()
                .map(|p| !dir_map.contains_key(&p.to_string_lossy().to_string()))
                .unwrap_or(true);
            if is_root {
                roots.push(build_dir_node(path, dir, &dir_map, &files_by_dir));
            }
        }

        if roots.is_empty() {
            for (path, dir) in dir_map.iter() {
                roots.push(build_dir_node(path, dir, &dir_map, &files_by_dir));
            }
        }

        roots
    }

    /// Clean path prefixes like "./" from refactoring candidates
    fn clean_path_prefixes(
        &self,
        candidates: &[RefactoringCandidate],
    ) -> Vec<RefactoringCandidate> {
        candidates
            .iter()
            .cloned()
            .map(|mut candidate| {
                // Clean the file_path
                candidate.file_path = self.clean_path_string(&candidate.file_path);

                // Clean the entity_id
                candidate.entity_id = self.clean_path_string(&candidate.entity_id);

                // Clean the name field if it also has the prefix
                candidate.name = self.clean_path_string(&candidate.name);

                candidate
            })
            .collect()
    }

    fn clean_entity_refs(&self, entities: &[RefactoringCandidate]) -> Vec<RefactoringCandidate> {
        entities
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.entity_id = self.clean_path_string(&entity.entity_id);
                entity.name = self.clean_path_string(&entity.name);
                entity.file_path = self.clean_path_string(&entity.file_path);
                entity
            })
            .collect()
    }

    /// Clean path strings by removing absolute path prefixes and "./" prefixes
    fn clean_path_string(&self, path: &str) -> String {
        // First handle absolute paths by converting to relative
        if let Ok(current_dir) = std::env::current_dir() {
            let current_dir_str = current_dir.to_string_lossy();
            if path.starts_with(&current_dir_str.as_ref()) {
                let relative = &path[current_dir_str.len()..];
                let cleaned = relative.strip_prefix('/').unwrap_or(relative);
                return cleaned.to_string();
            }
        }

        // Then handle "./" prefixes
        if path.starts_with("./") {
            path[2..].to_string()
        } else {
            path.to_string()
        }
    }

    fn clean_path_prefixes_in_file_groups(
        &self,
        file_groups: &[FileRefactoringGroup],
    ) -> Vec<FileRefactoringGroup> {
        file_groups
            .iter()
            .cloned()
            .map(|mut group| {
                // Clean the file_path
                group.file_path = self.clean_path_string(&group.file_path);

                // Clean all entities within the group
                group.entities = self.clean_entity_refs(&group.entities);

                group
            })
            .collect()
    }

    /// Clean "./" prefixes from directory health tree paths
    fn clean_directory_health_tree_paths(&self, tree: &DirectoryHealthTree) -> DirectoryHealthTree {
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

    /// Create real file groups from individual refactoring candidates
    fn create_file_groups_from_candidates(
        &self,
        candidates: &[RefactoringCandidate],
    ) -> Vec<FileRefactoringGroup> {
        use std::collections::HashMap;

        let mut file_map: HashMap<String, Vec<&RefactoringCandidate>> = HashMap::new();

        // Group candidates by file path
        for candidate in candidates {
            file_map
                .entry(candidate.file_path.clone())
                .or_insert_with(Vec::new)
                .push(candidate);
        }

        // Convert to FileRefactoringGroup format
        file_map
            .into_iter()
            .map(|(file_path, candidates)| {
                let file_name = std::path::Path::new(&file_path)
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("unknown"))
                    .to_string_lossy()
                    .to_string();

                let entity_count = candidates.len();
                let avg_score = if entity_count > 0 {
                    candidates.iter().map(|c| c.score).sum::<f64>() / entity_count as f64
                } else {
                    0.0
                };

                let highest_priority = candidates
                    .iter()
                    .map(|c| &c.priority)
                    .max()
                    .cloned()
                    .unwrap_or(Priority::Low);

                let total_issues = candidates.iter().map(|c| c.issue_count).sum::<usize>();

                // Use the candidates directly as entities
                let entities: Vec<RefactoringCandidate> = candidates.into_iter().cloned().collect();

                FileRefactoringGroup {
                    file_path,
                    file_name,
                    entity_count,
                    entities,
                    avg_score,
                    highest_priority,
                    total_issues,
                }
            })
            .collect()
    }

    fn build_candidate_lookup(
        &self,
        candidates: &[RefactoringCandidate],
    ) -> HashMap<String, RefactoringCandidate> {
        let mut map = HashMap::with_capacity(candidates.len());
        for candidate in candidates {
            map.insert(candidate.entity_id.clone(), candidate.clone());
        }
        map
    }

    /// Merge file data into the hierarchical directory structure
    fn add_files_to_hierarchy(
        &self,
        hierarchy: &[serde_json::Value],
        file_groups: &[FileRefactoringGroup],
        code_dictionary: &CodeDictionary,
        candidate_lookup: &HashMap<String, RefactoringCandidate>,
    ) -> Vec<serde_json::Value> {
        use std::collections::HashMap;
        use std::path::Path;

        // Build a map of directory path -> file groups for quick lookup
        let mut files_by_dir: HashMap<String, Vec<&FileRefactoringGroup>> = HashMap::new();

        for file_group in file_groups {
            let file_path = Path::new(&file_group.file_path);
            let dir_path = if let Some(parent) = file_path.parent() {
                parent.to_string_lossy().to_string()
            } else {
                ".".to_string()
            };

            files_by_dir
                .entry(dir_path)
                .or_insert_with(Vec::new)
                .push(file_group);
        }

        // Recursively add files to hierarchy nodes
        hierarchy
            .iter()
            .map(|node| {
                self.add_files_to_node(node, &files_by_dir, code_dictionary, candidate_lookup)
            })
            .collect()
    }

    /// Recursively add files to a single hierarchy node
    fn add_files_to_node(
        &self,
        node: &serde_json::Value,
        files_by_dir: &HashMap<String, Vec<&FileRefactoringGroup>>,
        code_dictionary: &CodeDictionary,
        candidate_lookup: &HashMap<String, RefactoringCandidate>,
    ) -> serde_json::Value {
        let mut new_node = node.clone();

        // Get the path from the node
        let node_path = if let Some(path) = node.get("path").and_then(|p| p.as_str()) {
            path.to_string()
        } else if let Some(id) = node.get("id").and_then(|id| id.as_str()) {
            // Extract path from ID like "directory_src_detectors" -> "src/detectors"
            if id.starts_with("directory_") {
                id.strip_prefix("directory_")
                    .unwrap_or(id)
                    .replace("_", "/")
                    .replace("root", ".")
            } else {
                ".".to_string()
            }
        } else {
            ".".to_string()
        };

        // Get existing children or create empty array
        let existing_children = node
            .get("children")
            .and_then(|c| c.as_array())
            .cloned()
            .unwrap_or_default();

        // Recursively process existing children (directories)
        let mut new_children: Vec<serde_json::Value> = existing_children
            .iter()
            .map(|child| {
                self.add_files_to_node(child, files_by_dir, code_dictionary, candidate_lookup)
            })
            .collect();

        // Add files that belong to this directory
        if let Some(file_groups) = files_by_dir.get(&node_path) {
            for file_group in file_groups {
                let file_name = Path::new(&file_group.file_path)
                    .file_name()
                    .unwrap_or_else(|| std::ffi::OsStr::new("unknown"))
                    .to_string_lossy()
                    .to_string();

                // Create file node with entity children
                let mut file_children = Vec::new();

                for entity in &file_group.entities {
                    // Extract entity name for better readability
                    let display_name = entity
                        .name
                        .split(':')
                        .last()
                        .map(|part| part.to_string())
                        .unwrap_or_else(|| entity.name.clone());

                    // Create children for issues and suggestions
                    let mut entity_children = Vec::new();

                    if let Some(candidate) = candidate_lookup.get(&entity.entity_id) {
                        for (i, issue) in candidate.issues.iter().enumerate() {
                            let issue_meta = code_dictionary.issues.get(&issue.code);
                            let issue_title = issue_meta
                                .map(|def| def.title.clone())
                                .unwrap_or_else(|| issue.category.clone());
                            let issue_summary = issue_meta
                                .map(|def| def.summary.clone())
                                .unwrap_or_else(|| {
                                    format!("{} signals detected by analyzer.", issue.category)
                                });
                            let severity = (issue.severity * 10.0).round() / 10.0;

                            entity_children.push(serde_json::json!({
                                "id": format!("{}:issue:{}", entity.entity_id, i),
                                "type": "issue",
                                "code": issue.code,
                                "name": format!("{} – {}", issue.code, issue_title),
                                "title": issue_title,
                                "category": issue.category,
                                "summary": issue_summary,
                                "severity": severity,
                                "contributing_features": issue.contributing_features,
                                "children": []
                            }));
                        }

                        for (i, suggestion) in candidate.suggestions.iter().enumerate() {
                            let suggestion_meta = code_dictionary.suggestions.get(&suggestion.code);
                            let suggestion_title = suggestion_meta
                                .map(|def| def.title.clone())
                                .unwrap_or_else(|| suggestion.refactoring_type.clone());
                            let suggestion_summary = suggestion_meta
                                .map(|def| def.summary.clone())
                                .unwrap_or_else(|| suggestion.refactoring_type.replace('_', " "));

                            entity_children.push(serde_json::json!({
                                "id": format!("{}:suggestion:{}", entity.entity_id, i),
                                "type": "suggestion",
                                "code": suggestion.code,
                                "name": format!("{} – {}", suggestion.code, suggestion_title),
                                "title": suggestion_title,
                                "summary": suggestion_summary,
                                "priority": ((suggestion.priority * 10.0).round() / 10.0),
                                "effort": ((suggestion.effort * 10.0).round() / 10.0),
                                "impact": ((suggestion.impact * 10.0).round() / 10.0),
                                "refactoring_type": suggestion.refactoring_type.clone(),
                                "children": []
                            }));
                        }
                    }

                    let entity_node = serde_json::json!({
                        "id": entity.entity_id.clone(),
                        "type": "entity",
                        "name": display_name,
                        "score": ((entity.score * 10.0).round() / 10.0),
                        "priority": format!("{:?}", entity.priority),
                        "issue_count": entity.issue_count,
                        "suggestion_count": entity.suggestion_count,
                        "children": entity_children
                    });
                    file_children.push(entity_node);
                }

                let file_node = serde_json::json!({
                    "id": format!("file_{}", file_group.file_path.replace("/", "_").replace(".", "root")),
                    "type": "file",
                    "name": file_name,
                    "path": file_group.file_path,
                    "entity_count": file_group.entity_count,
                    "avg_score": ((file_group.avg_score * 10.0).round() / 10.0),
                    "highest_priority": format!("{:?}", file_group.highest_priority),
                    "total_issues": file_group.total_issues,
                    "children": file_children
                });

                new_children.push(file_node);
            }
        }

        // Update the node with new children
        if let serde_json::Value::Object(ref mut obj) = new_node {
            obj.insert(
                "children".to_string(),
                serde_json::Value::Array(new_children),
            );
        }

        new_node
    }
}


#[cfg(test)]
#[path = "generator_tests.rs"]
mod tests;
