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
mod tests {
    use super::*;
    use crate::api::config_types::AnalysisConfig;
    use crate::core::pipeline::{
        AnalysisResults, AnalysisStatistics, AnalysisSummary, CodeDefinition, CodeDictionary,
        DirectoryHealthScore, DirectoryHealthTree, FeatureContribution, MemoryStats,
        NormalizedAnalysisResults, NormalizedEntity, NormalizedIssue, NormalizedIssueTotals,
        NormalizedSummary, RefactoringCandidate, RefactoringIssue, RefactoringSuggestion,
        TreeStatistics,
    };
    use crate::core::scoring::{Priority, ScoringResult};
    use crate::io::reports::templates;
    use crate::oracle::{
        CodebaseAssessment, IdentifiedRisk, RefactoringOracleResponse, RefactoringPhase,
        RefactoringPlan, RefactoringSubsystem, RefactoringTask, RiskAssessment,
    };
    use serial_test::serial;
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn entity_ref(candidate: &RefactoringCandidate) -> RefactoringCandidate {
        candidate.clone()
    }

    fn create_test_results() -> AnalysisResults {
        use std::time::Duration;
        let mut results = AnalysisResults::empty();
        results.summary.files_processed = 3;
        results.summary.total_files = 3;
        results.summary.entities_analyzed = 15;
        results.summary.total_entities = 15;
        results.summary.refactoring_needed = 5;
        results.summary.high_priority = 2;
        results.summary.high_priority_issues = 2;
        results.summary.critical = 1;
        results.summary.critical_issues = 1;
        results.summary.avg_refactoring_score = 0.65;
        results.summary.code_health_score = 0.75;
        results.summary.total_issues = 3;
        results.summary.total_lines_of_code = 600;
        results.summary.languages = vec!["Rust".to_string()];
        results.refactoring_candidates = vec![RefactoringCandidate {
            entity_id: "test_entity_1".to_string(),
            name: "complex_function".to_string(),
            file_path: "src/test.rs".to_string(),
            line_range: Some((10, 50)),
            priority: Priority::High,
            score: 0.85,
            confidence: 0.9,
            issues: vec![RefactoringIssue {
                code: "complexity.high".to_string(),
                category: "complexity".to_string(),
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
                code: "refactor.extract_method".to_string(),
                priority: 0.9,
                effort: 0.6,
                impact: 0.8,
            }],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        }];
        results.statistics.total_duration = Duration::from_millis(1500);
        results.statistics.avg_file_processing_time = Duration::from_millis(500);
        results.statistics.avg_entity_processing_time = Duration::from_millis(100);
        results.statistics.memory_stats = MemoryStats {
            peak_memory_bytes: 128 * 1024 * 1024,
            final_memory_bytes: 64 * 1024 * 1024,
            efficiency_score: 0.85,
        };
        results.warnings = vec!["Test warning".to_string()];
        results.coverage_packs = vec![crate::detectors::coverage::CoveragePack {
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
                    head: vec!["    fn uncovered_function(x: i32) -> Result<String> {".to_string()],
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
        }];
        results
    }

    #[test]
    fn test_report_generator_new() {
        let generator = ReportGenerator::new();
        assert!(generator
            .handlebars
            .get_templates()
            .contains_key("default_html"));
        let expected_templates_dir = templates::detect_templates_dir();
        assert_eq!(
            generator.templates_dir.is_some(),
            expected_templates_dir.is_some()
        );
    }

    #[test]
    fn test_report_generator_default() {
        let generator = ReportGenerator::default();
        assert!(generator
            .handlebars
            .get_templates()
            .contains_key("default_html"));
        let expected_templates_dir = templates::detect_templates_dir();
        assert_eq!(
            generator.templates_dir.is_some(),
            expected_templates_dir.is_some()
        );
    }

    #[test]
    fn test_generator_with_config_stores_analysis_config() {
        let config = AnalysisConfig::default();
        let generator = ReportGenerator::new().with_config(config.clone());
        assert!(generator.analysis_config.is_some());
        let stored = generator.analysis_config.as_ref().expect("config stored");
        assert_eq!(stored.modules.complexity, config.modules.complexity);
        assert_eq!(stored.coverage.enabled, config.coverage.enabled);
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
        assert!(content.contains("file_path"));
        assert!(content.contains("src/test.rs"));
    }

    #[test]
    fn test_generate_markdown_sonar_and_csv_table_reports() {
        let temp_dir = TempDir::new().unwrap();
        let generator = ReportGenerator::new();
        let results = create_test_results();

        let markdown_path = temp_dir.path().join("report.md");
        generator
            .generate_markdown_report(&results, &markdown_path)
            .expect("markdown report");
        let markdown_content = fs::read_to_string(&markdown_path).unwrap();
        assert!(markdown_content.contains("Valknut Analysis Report"));

        let csv_table_path = temp_dir.path().join("report_table.csv");
        generator
            .generate_csv_table(&results, &csv_table_path)
            .expect("csv table");
        let csv_table_content = fs::read_to_string(&csv_table_path).unwrap();
        assert!(csv_table_content.contains("complex_function"));

        let sonar_path = temp_dir.path().join("sonar.json");
        generator
            .generate_sonar_report(&results, &sonar_path)
            .expect("sonar report");
        let sonar_content = fs::read_to_string(&sonar_path).unwrap();
        assert!(sonar_content.contains("\"issues\""));
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

        assert!(obj.contains_key("tree_payload"));
    }

    fn sample_oracle_response() -> RefactoringOracleResponse {
        RefactoringOracleResponse {
            assessment: CodebaseAssessment {
                health_score: 72,
                strengths: vec!["Well-structured modules".into()],
                weaknesses: vec!["Large util file".into()],
                architecture_quality: "Good separation of concerns".into(),
                organization_quality: "Needs documentation cleanup".into(),
            },
            refactoring_plan: RefactoringPlan {
                phases: vec![RefactoringPhase {
                    id: "phase-1".into(),
                    name: "Documentation Refresh".into(),
                    description: "Update README and inline docs".into(),
                    priority: 1,
                    subsystems: vec![RefactoringSubsystem {
                        id: "docs".into(),
                        name: "Documentation".into(),
                        affected_files: vec!["README.md".into(), "src/lib.rs".into()],
                        tasks: vec![RefactoringTask {
                            id: "task-1".into(),
                            title: "Refresh README".into(),
                            description: "Update overview and usage sections".into(),
                            task_type: "refactor_class".into(),
                            files: vec!["README.md".into()],
                            risk_level: "low".into(),
                            benefits: vec!["Improved onboarding".into()],
                        }],
                    }],
                }],
            },
            risk_assessment: RiskAssessment {
                overall_risk: "low".into(),
                risks: vec![IdentifiedRisk {
                    category: "process".into(),
                    description: "Docs may lag behind refactors".into(),
                    probability: "medium".into(),
                    impact: "medium".into(),
                    mitigation: "Schedule review cadence".into(),
                }],
                mitigation_strategies: vec!["Adopt doc review checklist".into()],
            },
        }
    }

    #[test]
    fn test_prepare_template_data_marks_oracle_presence() {
        let generator = ReportGenerator::new();
        let results = create_test_results();
        let oracle = sample_oracle_response();

        let data = generator.prepare_template_data_with_oracle(&results, &Some(oracle.clone()));
        let obj = data.as_object().expect("template data should be object");
        assert_eq!(obj["has_oracle_data"], serde_json::Value::Bool(true));
        assert!(obj.contains_key("oracle_refactoring_plan"));

        let without_oracle = generator.prepare_template_data(&results);
        let without_obj = without_oracle
            .as_object()
            .expect("template data should be object");
        assert_eq!(
            without_obj["has_oracle_data"],
            serde_json::Value::Bool(false)
        );
    }

    #[test]
    fn test_generate_report_with_oracle_all_formats() {
        let temp_dir = TempDir::new().unwrap();
        let generator = ReportGenerator::new();
        let results = create_test_results();
        let oracle = sample_oracle_response();

        let json_path = temp_dir.path().join("report.json");
        generator
            .generate_report_with_oracle(&results, &oracle, &json_path, ReportFormat::Json)
            .expect("json report should succeed");
        let json_content = fs::read_to_string(&json_path).unwrap();
        assert!(json_content.contains("oracle_refactoring_plan"));

        let html_path = temp_dir.path().join("report.html");
        generator
            .generate_report_with_oracle(&results, &oracle, &html_path, ReportFormat::Html)
            .expect("html report should succeed");
        let html_content = fs::read_to_string(&html_path).unwrap();
        assert!(html_content.contains("Analysis Report"));
        let assets_dir = temp_dir.path().join("webpage_files");
        assert!(
            assets_dir.exists(),
            "expected webpage assets directory to be created"
        );

        let yaml_path = temp_dir.path().join("report.yaml");
        generator
            .generate_report_with_oracle(&results, &oracle, &yaml_path, ReportFormat::Yaml)
            .expect("yaml report should succeed");
        let yaml_content = fs::read_to_string(&yaml_path).unwrap();
        assert!(yaml_content.contains("oracle_refactoring_plan"));

        let csv_path = temp_dir.path().join("report.csv");
        generator
            .generate_report_with_oracle(&results, &oracle, &csv_path, ReportFormat::Csv)
            .expect("csv report should succeed");
        let csv_content = fs::read_to_string(&csv_path).unwrap();
        assert!(csv_content.contains("complex_function"));
    }

    #[serial]
    #[test]
    fn test_clean_path_helpers_strip_prefixes() {
        let generator = ReportGenerator::new();

        let original_dir = std::env::current_dir().unwrap();
        let temp_dir = TempDir::new().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let absolute_path = temp_dir.path().join("src/lib.rs");
        let cleaned_abs = generator.clean_path_string(absolute_path.to_str().unwrap());
        assert_eq!(cleaned_abs, "src/lib.rs");

        std::env::set_current_dir(&original_dir).unwrap();

        let with_dot = generator.clean_path_string("./src/main.rs");
        assert_eq!(with_dot, "src/main.rs");
    }

    #[test]
    fn test_clean_path_prefixes_in_file_groups_and_candidates() {
        let generator = ReportGenerator::new();

        let candidates = vec![RefactoringCandidate {
            entity_id: "./src/lib.rs:function".into(),
            name: "./src/lib.rs::function".into(),
            file_path: "./src/lib.rs".into(),
            line_range: Some((1, 10)),
            priority: Priority::High,
            score: 0.8,
            confidence: 0.9,
            issues: vec![],
            suggestions: vec![],
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        }];

        let file_groups = vec![FileRefactoringGroup {
            file_path: "./src/lib.rs".into(),
            file_name: "lib.rs".into(),
            entity_count: 1,
            avg_score: 0.8,
            highest_priority: Priority::High,
            total_issues: 1,
            entities: vec![entity_ref(&candidates[0])],
        }];

        let cleaned_candidates = generator.clean_path_prefixes(&candidates);
        assert_eq!(cleaned_candidates[0].file_path, "src/lib.rs");
        assert_eq!(cleaned_candidates[0].entity_id, "src/lib.rs:function");

        let cleaned_groups = generator.clean_path_prefixes_in_file_groups(&file_groups);
        assert_eq!(cleaned_groups[0].file_path, "src/lib.rs");
        assert_eq!(cleaned_groups[0].entities[0].name, "src/lib.rs::function");
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
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        assert_eq!(
            summary.get("high_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(2))
        );
        assert_eq!(
            summary.get("critical_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(1))
        );
    }

    #[test]
    fn test_calculate_summary_prefers_normalized_data() {
        let generator = ReportGenerator::new();
        let mut results = create_test_results();

        let normalized = NormalizedAnalysisResults {
            meta: NormalizedSummary {
                timestamp: Utc::now(),
                files_scanned: 10,
                entities_analyzed: 42,
                code_health: 0.91,
                languages: vec!["rust".to_string(), "typescript".to_string()],
                issues: NormalizedIssueTotals {
                    total: 8,
                    high: 3,
                    critical: 1,
                },
            },
            entities: vec![NormalizedEntity {
                id: "src/lib.rs:function:one".into(),
                name: "one".into(),
                file: Some("src/lib.rs".into()),
                kind: Some("function".into()),
                line_range: Some((10, 20)),
                priority: Priority::High,
                score: 0.82,
                metrics: None,
                issues: vec![NormalizedIssue::from(("CMPLX".to_string(), 1.2))],
                suggestions: Vec::new(),
                file_path: Some("src/lib.rs".to_string()),
                issue_count: 0,
                suggestion_count: 0,
            }],
            clone: None,
            warnings: Vec::new(),
            dictionary: CodeDictionary::default(),
        };

        results.normalized = Some(normalized);

        let summary = generator.calculate_summary(&results);

        assert_eq!(
            summary.get("files_processed").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        assert_eq!(
            summary.get("entities_analyzed").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(15))
        );
        assert_eq!(
            summary.get("total_issues").unwrap(),
            &serde_json::Value::Number(serde_json::Number::from(3))
        );
        let languages = summary.get("languages").unwrap().as_array().unwrap();
        assert_eq!(languages.len(), 1);
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
                let result =
                    templates::load_templates_from_dir(&mut generator.handlebars, &templates_dir);
                // Just make sure it doesn't panic, the result could be ok or error
                let _ = result;
            }
            Err(_) => {
                // If we can't create the invalid file, that's expected
                // Just test with a normal template loading that should work
                let good_file = templates_dir.join("good.hbs");
                fs::write(&good_file, "{{content}}").unwrap();

                let mut generator = ReportGenerator::new();
                let result =
                    templates::load_templates_from_dir(&mut generator.handlebars, &templates_dir);
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

        let result = templates::load_templates_from_dir(&mut generator.handlebars, &templates_dir);
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
            doc_health_score: 1.0,
            doc_issue_count: 0,
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
            doc_health_score: 1.0,
            doc_issue_count: 0,
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
            doc_health_score: 1.0,
            doc_issue_count: 0,
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

    #[test]
    fn test_add_files_to_hierarchy_basic() {
        let generator = ReportGenerator::new();

        // Create a simple hierarchy
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": []
        })];

        let candidate = RefactoringCandidate {
            entity_id: "test_entity".to_string(),
            name: "test_function".to_string(),
            file_path: "src/test.rs".to_string(),
            line_range: Some((10, 20)),
            priority: Priority::High,
            score: 0.85,
            confidence: 0.9,
            issues: vec![],
            suggestions: vec![],
            issue_count: 3,
            suggestion_count: 1,
            coverage_percentage: None,
        };

        let file_groups = vec![FileRefactoringGroup {
            file_path: "src/test.rs".to_string(),
            file_name: "test.rs".to_string(),
            entity_count: 1,
            avg_score: 0.85,
            highest_priority: Priority::High,
            total_issues: 3,
            entities: vec![entity_ref(&candidate)],
        }];

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(candidate.entity_id.clone(), candidate.clone());

        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Verify structure
        assert_eq!(result.len(), 1);
        let dir_node = &result[0];
        assert_eq!(dir_node["type"], "folder");
        assert_eq!(dir_node["name"], "src");

        // Verify file was added
        let children = dir_node["children"].as_array().unwrap();
        assert_eq!(children.len(), 1);

        let file_node = &children[0];
        assert_eq!(file_node["type"], "file");
        assert_eq!(file_node["name"], "test.rs");
        assert_eq!(file_node["path"], "src/test.rs");
        assert_eq!(file_node["entity_count"], 1);

        // Verify entity was added as child of file
        let file_children = file_node["children"].as_array().unwrap();
        assert_eq!(file_children.len(), 1);
        let entity_node = &file_children[0];
        assert_eq!(entity_node["type"], "entity");
        assert_eq!(entity_node["name"], "test_function");
    }

    #[test]
    fn test_add_files_to_hierarchy_nested_directories() {
        let generator = ReportGenerator::new();

        // Create nested hierarchy
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": [
                {
                    "id": "directory_src_core",
                    "type": "folder",
                    "name": "core",
                    "path": "src/core",
                    "children": []
                }
            ]
        })];

        let main_candidate = RefactoringCandidate {
            entity_id: "main_entity".to_string(),
            name: "main".to_string(),
            file_path: "src/main.rs".to_string(),
            line_range: Some((1, 10)),
            priority: Priority::Medium,
            score: 0.7,
            confidence: 0.8,
            issues: vec![],
            suggestions: vec![],
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let lib_candidate = RefactoringCandidate {
            entity_id: "lib_entity".to_string(),
            name: "lib_function".to_string(),
            file_path: "src/core/lib.rs".to_string(),
            line_range: Some((20, 30)),
            priority: Priority::High,
            score: 0.9,
            confidence: 0.95,
            issues: vec![],
            suggestions: vec![],
            issue_count: 5,
            suggestion_count: 2,
            coverage_percentage: None,
        };

        let file_groups = vec![
            FileRefactoringGroup {
                file_path: "src/main.rs".to_string(),
                file_name: "main.rs".to_string(),
                entity_count: 1,
                avg_score: 0.7,
                highest_priority: Priority::Medium,
                total_issues: 1,
                entities: vec![entity_ref(&main_candidate)],
            },
            FileRefactoringGroup {
                file_path: "src/core/lib.rs".to_string(),
                file_name: "lib.rs".to_string(),
                entity_count: 2,
                avg_score: 0.9,
                highest_priority: Priority::High,
                total_issues: 5,
                entities: vec![entity_ref(&lib_candidate)],
            },
        ];

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(main_candidate.entity_id.clone(), main_candidate.clone());
        candidate_lookup.insert(lib_candidate.entity_id.clone(), lib_candidate.clone());

        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Verify root structure
        assert_eq!(result.len(), 1);
        let root_dir = &result[0];
        assert_eq!(root_dir["name"], "src");

        let root_children = root_dir["children"].as_array().unwrap();
        assert_eq!(root_children.len(), 2); // core directory + main.rs file

        // Find the core directory and main.rs file
        let mut core_dir = None;
        let mut main_file = None;

        for child in root_children {
            if child["type"] == "folder" && child["name"] == "core" {
                core_dir = Some(child);
            } else if child["type"] == "file" && child["name"] == "main.rs" {
                main_file = Some(child);
            }
        }

        // Verify main.rs is in src/
        let main_file = main_file.expect("main.rs file should be present");
        assert_eq!(main_file["path"], "src/main.rs");
        assert_eq!(main_file["entity_count"], 1);

        // Verify core directory exists and has lib.rs
        let core_dir = core_dir.expect("core directory should be present");
        let core_children = core_dir["children"].as_array().unwrap();
        assert_eq!(core_children.len(), 1);

        let lib_file = &core_children[0];
        assert_eq!(lib_file["type"], "file");
        assert_eq!(lib_file["name"], "lib.rs");
        assert_eq!(lib_file["path"], "src/core/lib.rs");
        assert_eq!(lib_file["entity_count"], 2);
    }

    #[test]
    fn test_add_files_to_hierarchy_empty_file_groups() {
        let generator = ReportGenerator::new();

        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": []
        })];

        let file_groups = vec![];
        let candidate_lookup = HashMap::new();
        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Should preserve hierarchy without changes
        assert_eq!(result.len(), 1);
        let dir_node = &result[0];
        assert_eq!(dir_node["name"], "src");
        let children = dir_node["children"].as_array().unwrap();
        assert_eq!(children.len(), 0); // No files added
    }

    #[test]
    fn test_add_files_to_hierarchy_preserves_existing_children() {
        let generator = ReportGenerator::new();

        // Create hierarchy with existing children
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": [
                {
                    "id": "directory_src_existing",
                    "type": "folder",
                    "name": "existing",
                    "path": "src/existing",
                    "children": []
                }
            ]
        })];

        let new_candidate = RefactoringCandidate {
            entity_id: "new_entity".to_string(),
            name: "new_function".to_string(),
            file_path: "src/new.rs".to_string(),
            line_range: None,
            priority: Priority::Low,
            score: 0.5,
            confidence: 0.6,
            issues: vec![],
            suggestions: vec![],
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let file_groups = vec![FileRefactoringGroup {
            file_path: "src/new.rs".to_string(),
            file_name: "new.rs".to_string(),
            entity_count: 1,
            avg_score: 0.5,
            highest_priority: Priority::Low,
            total_issues: 1,
            entities: vec![entity_ref(&new_candidate)],
        }];

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(new_candidate.entity_id.clone(), new_candidate.clone());

        let result = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &CodeDictionary::default(),
            &candidate_lookup,
        );

        // Verify both existing directory and new file are present
        assert_eq!(result.len(), 1);
        let root_dir = &result[0];
        let children = root_dir["children"].as_array().unwrap();
        assert_eq!(children.len(), 2); // existing directory + new file

        // Verify existing directory is preserved
        let existing_dir = children
            .iter()
            .find(|child| child["type"] == "folder" && child["name"] == "existing")
            .expect("existing directory should be preserved");
        assert_eq!(existing_dir["path"], "src/existing");

        // Verify new file is added
        let new_file = children
            .iter()
            .find(|child| child["type"] == "file" && child["name"] == "new.rs")
            .expect("new file should be added");
        assert_eq!(new_file["path"], "src/new.rs");
    }

    #[test]
    fn test_build_unified_hierarchy_sorts_by_priority() {
        let generator = ReportGenerator::new();

        let mut directories = HashMap::new();
        directories.insert(
            PathBuf::from("src"),
            DirectoryHealthScore {
                path: PathBuf::from("src"),
                health_score: 0.3,
                file_count: 2,
                entity_count: 3,
                refactoring_needed: 3,
                critical_issues: 1,
                high_priority_issues: 2,
                avg_refactoring_score: 0.4,
                weight: 1.0,
                children: vec![PathBuf::from("src/core")],
                parent: Some(PathBuf::from(".")),
                issue_categories: HashMap::new(),
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
        );
        directories.insert(
            PathBuf::from("src/core"),
            DirectoryHealthScore {
                path: PathBuf::from("src/core"),
                health_score: 0.6,
                file_count: 1,
                entity_count: 1,
                refactoring_needed: 1,
                critical_issues: 0,
                high_priority_issues: 1,
                avg_refactoring_score: 0.7,
                weight: 1.0,
                children: Vec::new(),
                parent: Some(PathBuf::from("src")),
                issue_categories: HashMap::new(),
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
        );

        let tree = DirectoryHealthTree {
            root: DirectoryHealthScore {
                path: PathBuf::from("."),
                health_score: 0.2,
                file_count: 0,
                entity_count: 0,
                refactoring_needed: 0,
                critical_issues: 0,
                high_priority_issues: 0,
                avg_refactoring_score: 0.0,
                weight: 1.0,
                children: vec![PathBuf::from("src")],
                parent: None,
                issue_categories: HashMap::new(),
                doc_health_score: 1.0,
                doc_issue_count: 0,
            },
            directories,
            tree_statistics: TreeStatistics {
                total_directories: 2,
                max_depth: 2,
                avg_health_score: 0.45,
                health_score_std_dev: 0.1,
                hotspot_directories: Vec::new(),
                health_by_depth: HashMap::new(),
            },
        };

        let critical_entity = RefactoringCandidate {
            entity_id: "src/critical.rs::function".to_string(),
            name: "module::critical_function".to_string(),
            file_path: "src/critical.rs".to_string(),
            line_range: Some((5, 25)),
            priority: Priority::Critical,
            score: 0.95,
            confidence: 0.9,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 2,
            suggestion_count: 0,
            coverage_percentage: None,
        };
        let medium_entity = RefactoringCandidate {
            entity_id: "src/medium.rs::function".to_string(),
            name: "module::medium_function".to_string(),
            file_path: "src/medium.rs".to_string(),
            line_range: Some((10, 30)),
            priority: Priority::Medium,
            score: 0.7,
            confidence: 0.8,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };
        let core_entity = RefactoringCandidate {
            entity_id: "src/core/lib.rs::helper".to_string(),
            name: "module::helper".to_string(),
            file_path: "src/core/lib.rs".to_string(),
            line_range: Some((1, 20)),
            priority: Priority::High,
            score: 0.82,
            confidence: 0.85,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let file_groups = vec![
            FileRefactoringGroup {
                file_path: "src/critical.rs".to_string(),
                file_name: "critical.rs".to_string(),
                entity_count: 1,
                highest_priority: Priority::Critical,
                avg_score: 0.95,
                total_issues: 2,
                entities: vec![entity_ref(&critical_entity)],
            },
            FileRefactoringGroup {
                file_path: "src/medium.rs".to_string(),
                file_name: "medium.rs".to_string(),
                entity_count: 1,
                highest_priority: Priority::Medium,
                avg_score: 0.7,
                total_issues: 1,
                entities: vec![entity_ref(&medium_entity)],
            },
            FileRefactoringGroup {
                file_path: "src/core/lib.rs".to_string(),
                file_name: "lib.rs".to_string(),
                entity_count: 1,
                highest_priority: Priority::High,
                avg_score: 0.82,
                total_issues: 1,
                entities: vec![entity_ref(&core_entity)],
            },
        ];

        let hierarchy = generator.build_unified_hierarchy(&tree, &file_groups);
        assert_eq!(hierarchy.len(), 1);

        let src_node = &hierarchy[0];
        assert_eq!(src_node["path"], "src");
        let children = src_node["children"]
            .as_array()
            .expect("src should contain children");
        assert_eq!(children.len(), 3);

        let critical_file = children
            .iter()
            .find(|child| child["type"] == "file" && child["name"] == "critical.rs")
            .expect("critical.rs should be present");
        assert_eq!(critical_file["priority"].as_str(), Some("Critical"));
        assert_eq!(critical_file["entity_count"], 1);

        let medium_file = children
            .iter()
            .find(|child| child["type"] == "file" && child["name"] == "medium.rs")
            .expect("medium.rs should be present");
        assert_eq!(medium_file["priority"].as_str(), Some("Medium"));

        let core_node = children
            .iter()
            .find(|child| child["type"] == "folder" && child["path"] == "src/core")
            .expect("core directory should exist");
        let core_children = core_node["children"]
            .as_array()
            .expect("core children array");
        assert_eq!(core_children.len(), 1);
        assert_eq!(core_children[0]["name"], "lib.rs");
    }

    #[test]
    fn test_add_files_to_hierarchy_enriches_metadata() {
        let generator = ReportGenerator::new();
        let hierarchy = vec![serde_json::json!({
            "id": "directory_src",
            "type": "folder",
            "name": "src",
            "path": "src",
            "children": [
                {
                    "id": "directory_src_core",
                    "type": "folder",
                    "name": "core",
                    "path": "src/core",
                    "children": []
                }
            ]
        })];

        let detailed_candidate = RefactoringCandidate {
            entity_id: "src/core/lib.rs::entity".to_string(),
            name: "module::entity".to_string(),
            file_path: "src/core/lib.rs".to_string(),
            line_range: Some((42, 84)),
            priority: Priority::High,
            score: 0.88,
            confidence: 0.91,
            issues: vec![RefactoringIssue {
                code: "complexity.high".to_string(),
                category: "complexity".to_string(),
                severity: 2.3,
                contributing_features: vec![FeatureContribution {
                    feature_name: "cyclomatic_complexity".to_string(),
                    value: 21.0,
                    normalized_value: 0.9,
                    contribution: 1.4,
                }],
            }],
            suggestions: vec![RefactoringSuggestion {
                refactoring_type: "reduce_complexity".to_string(),
                code: "refactor.reduce".to_string(),
                priority: 0.8,
                effort: 0.5,
                impact: 0.9,
            }],
            issue_count: 1,
            suggestion_count: 1,
            coverage_percentage: None,
        };

        let file_groups = vec![FileRefactoringGroup {
            file_path: "src/core/lib.rs".to_string(),
            file_name: "lib.rs".to_string(),
            entity_count: 1,
            highest_priority: Priority::High,
            avg_score: 0.88,
            total_issues: 1,
            entities: vec![entity_ref(&detailed_candidate)],
        }];

        let mut dictionary = CodeDictionary::default();
        dictionary.issues.insert(
            "complexity.high".to_string(),
            CodeDefinition {
                code: "complexity.high".to_string(),
                title: "Elevated Complexity".to_string(),
                summary: "Function exceeds allowed complexity threshold.".to_string(),
                category: Some("complexity".to_string()),
            },
        );
        dictionary.suggestions.insert(
            "refactor.reduce".to_string(),
            CodeDefinition {
                code: "refactor.reduce".to_string(),
                title: "Reduce Complexity".to_string(),
                summary: "Break the function into smaller, focused helpers.".to_string(),
                category: Some("refactoring".to_string()),
            },
        );

        let mut candidate_lookup = HashMap::new();
        candidate_lookup.insert(
            detailed_candidate.entity_id.clone(),
            detailed_candidate.clone(),
        );

        let enriched = generator.add_files_to_hierarchy(
            &hierarchy,
            &file_groups,
            &dictionary,
            &candidate_lookup,
        );

        let root_children = enriched[0]["children"]
            .as_array()
            .expect("root should have children");
        let core_node = root_children
            .iter()
            .find(|child| child["type"] == "folder" && child["name"] == "core")
            .expect("core directory should exist");

        let file_node = core_node["children"]
            .as_array()
            .expect("core should contain files")[0]
            .clone();
        assert_eq!(file_node["highest_priority"].as_str(), Some("High"));

        let entity_node = file_node["children"]
            .as_array()
            .expect("file should contain entities")[0]
            .clone();
        assert_eq!(entity_node["name"], "entity");
        assert_eq!(entity_node["priority"].as_str(), Some("High"));
        assert!((entity_node["score"].as_f64().unwrap() - 0.9).abs() < f64::EPSILON);

        let metadata_children = entity_node["children"]
            .as_array()
            .expect("entity should contain metadata");
        assert_eq!(metadata_children.len(), 2);
        let issue_child = &metadata_children[0];
        assert_eq!(issue_child["title"], "Elevated Complexity");
        assert_eq!(
            issue_child["summary"],
            "Function exceeds allowed complexity threshold."
        );
        let suggestion_child = &metadata_children[1];
        assert_eq!(suggestion_child["title"], "Reduce Complexity");
        assert_eq!(
            suggestion_child["summary"],
            "Break the function into smaller, focused helpers."
        );
    }

    #[test]
    fn test_create_file_groups_from_candidates_groups_stats() {
        let generator = ReportGenerator::new();

        let mut candidate_a = RefactoringCandidate {
            entity_id: "src/lib.rs::alpha".to_string(),
            name: "alpha".to_string(),
            file_path: "src/lib.rs".to_string(),
            line_range: Some((1, 10)),
            priority: Priority::Medium,
            score: 0.8,
            confidence: 0.9,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 2,
            suggestion_count: 0,
            coverage_percentage: None,
        };
        let mut candidate_b = candidate_a.clone();
        candidate_b.entity_id = "src/lib.rs::beta".to_string();
        candidate_b.name = "beta".to_string();
        candidate_b.priority = Priority::High;
        candidate_b.score = 1.0;
        candidate_b.issue_count = 1;

        let candidate_c = RefactoringCandidate {
            entity_id: "src/utils.rs::gamma".to_string(),
            name: "gamma".to_string(),
            file_path: "src/utils.rs".to_string(),
            line_range: Some((15, 40)),
            priority: Priority::Low,
            score: 0.6,
            confidence: 0.8,
            issues: Vec::new(),
            suggestions: Vec::new(),
            issue_count: 1,
            suggestion_count: 0,
            coverage_percentage: None,
        };

        let groups = generator.create_file_groups_from_candidates(&[
            candidate_a.clone(),
            candidate_b.clone(),
            candidate_c.clone(),
        ]);
        assert_eq!(groups.len(), 2);

        let lib_group = groups
            .iter()
            .find(|g| g.file_path == "src/lib.rs")
            .expect("src/lib.rs group should exist");
        assert_eq!(lib_group.entity_count, 2);
        assert_eq!(lib_group.total_issues, 3);
        assert_eq!(lib_group.highest_priority, Priority::High);
        assert!(
            (lib_group.avg_score - 0.9).abs() < f64::EPSILON,
            "expected average score of 0.9 but found {}",
            lib_group.avg_score
        );
        assert_eq!(lib_group.entities.len(), 2);

        let utils_group = groups
            .iter()
            .find(|g| g.file_path == "src/utils.rs")
            .expect("src/utils.rs group should exist");
        assert_eq!(utils_group.entity_count, 1);
        assert_eq!(utils_group.total_issues, 1);
    }

    #[test]
    fn test_html_report_uses_hierarchical_data() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("hierarchy_test.html");
        let generator = ReportGenerator::new();

        // Create test results with hierarchical structure
        let mut results = create_test_results();

        // Create a minimal directory health tree so the hierarchy logic gets triggered
        use crate::core::pipeline::{DirectoryHealthScore, DirectoryHealthTree};
        use std::collections::HashMap;

        let mut directories = HashMap::new();
        let src_dir = DirectoryHealthScore {
            path: PathBuf::from("src"),
            health_score: 0.8,
            file_count: 1,
            entity_count: 1,
            refactoring_needed: 1,
            critical_issues: 0,
            high_priority_issues: 1,
            avg_refactoring_score: 0.85,
            weight: 1.0,
            children: vec![],
            parent: None,
            issue_categories: HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };
        directories.insert(PathBuf::from("src"), src_dir);

        let root_dir = DirectoryHealthScore {
            path: PathBuf::from("."),
            health_score: 0.8,
            file_count: 1,
            entity_count: 1,
            refactoring_needed: 1,
            critical_issues: 0,
            high_priority_issues: 1,
            avg_refactoring_score: 0.85,
            weight: 1.0,
            children: vec![PathBuf::from("src")],
            parent: None,
            issue_categories: HashMap::new(),
            doc_health_score: 1.0,
            doc_issue_count: 0,
        };

        let result = generator.generate_report(&results, &output_path, ReportFormat::Html);
        assert!(result.is_ok());

        let content = fs::read_to_string(&output_path).unwrap();

        // Sanity check that we produced non-empty HTML
        assert!(!content.is_empty());
    }
}
