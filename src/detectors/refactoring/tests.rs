use super::*;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

use crate::core::config::ValknutConfig;
use crate::core::featureset::{CodeEntity, ExtractionContext};

fn analyzer() -> RefactoringAnalyzer {
    RefactoringAnalyzer::new(RefactoringConfig::default(), Arc::new(AstService::new()))
}

#[test]
fn test_refactoring_config_default() {
    let config = RefactoringConfig::default();
    assert!(config.enabled);
    assert_eq!(config.min_impact_threshold, 5.0);
}

#[test]
fn test_refactoring_analyzer_creation() {
    let ast_service = Arc::new(AstService::new());
    let analyzer = RefactoringAnalyzer::new(RefactoringConfig::default(), ast_service);
    assert!(analyzer.config.enabled);
}

#[tokio::test]
async fn test_analyze_files_disabled() {
    let config = RefactoringConfig {
        enabled: false,
        min_impact_threshold: 5.0,
    };
    let analyzer = RefactoringAnalyzer::new(config, Arc::new(AstService::new()));

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.py");
    fs::write(&file_path, "def test_function():\n    pass").unwrap();

    let paths = vec![file_path];
    let results = analyzer.analyze_files(&paths).await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_detects_long_method() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("long_function.py");
    let mut content = String::from("def long_function():\n");
    for i in 0..65 {
        content.push_str(&format!("    value = {}\n", i));
    }
    fs::write(&file_path, content).unwrap();

    let analyzer = analyzer();
    let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
    assert_eq!(results.len(), 1);
    let has_extract_method = results[0]
        .recommendations
        .iter()
        .any(|rec| rec.refactoring_type == RefactoringType::ExtractMethod);
    assert!(has_extract_method, "Expected long method recommendation");
}

#[tokio::test]
async fn test_detects_complex_conditionals() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("complex_condition.py");
    let content = r#"
def complex_condition(a, b, c, d):
    if (a and b) or (c and d) or (a and c and d):
        return True
    return False
"#;
    fs::write(&file_path, content).unwrap();

    let analyzer = analyzer();
    let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
    assert_eq!(results.len(), 1);
    let has_complexity = results[0]
        .recommendations
        .iter()
        .any(|rec| rec.refactoring_type == RefactoringType::SimplifyConditionals);
    assert!(
        has_complexity,
        "Expected complex conditional recommendation"
    );
}

#[tokio::test]
async fn test_detects_duplicate_functions() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("duplicates.py");
    let content = r#"
def helper():
    total = 0
    for i in range(10):
        total += i * 2
        if total % 3 == 0:
            total -= 1
        else:
            total += 1
    return total

def helper_copy():
    total = 0
    for i in range(10):
        total += i * 2
        if total % 3 == 0:
            total -= 1
        else:
            total += 1
    return total
"#;
    fs::write(&file_path, content).unwrap();

    let analyzer = analyzer();
    let source = fs::read_to_string(&file_path).unwrap();
    let mut adapter = crate::lang::python::PythonAdapter::new().unwrap();
    let file_path_str = file_path.to_string_lossy().to_string();
    let parse_index = adapter.parse_source(&source, &file_path_str).unwrap();
    let ast_service = Arc::new(AstService::new());
    let cached_tree = ast_service.get_ast(&file_path_str, &source).await.unwrap();
    let ast_context = ast_service.create_context(&cached_tree, &file_path_str);
    let complexity_map = HashMap::<String, ComplexityAnalysisResult>::new();
    let summaries = analyzer
        .collect_entity_summaries(&parse_index, &source, &complexity_map, &ast_context)
        .unwrap();
    assert!(
        summaries
            .iter()
            .filter(|s| RefactoringAnalyzer::is_function_entity(s))
            .count()
            >= 2
    );
    let duplicate_ready = summaries
        .iter()
        .filter(|s| RefactoringAnalyzer::is_function_entity(s))
        .filter(|s| RefactoringAnalyzer::duplicate_signature(s).is_some())
        .count();
    assert!(
        duplicate_ready >= 2,
        "expected duplicate fingerprints to be present"
    );

    let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
    assert_eq!(results.len(), 1);
    let has_duplicate = results[0]
        .recommendations
        .iter()
        .any(|rec| rec.refactoring_type == RefactoringType::EliminateDuplication);
    assert!(has_duplicate, "Expected duplicate code recommendation");
}

#[tokio::test]
async fn test_detects_large_class() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("large_class.py");
    let mut content = String::from("class HugeClass:\n");
    for i in 0..30 {
        content.push_str(&format!("    def method_{}(self):\n", i));
        content.push_str("        result = 0\n");
        for j in 0..10 {
            content.push_str(&format!("        result += {}\n", j));
        }
        content.push_str("        return result\n\n");
    }
    fs::write(&file_path, content).unwrap();

    let analyzer = analyzer();
    let results = analyzer.analyze_files(&[file_path.clone()]).await.unwrap();
    assert_eq!(results.len(), 1);
    let has_large_class = results[0]
        .recommendations
        .iter()
        .any(|rec| rec.refactoring_type == RefactoringType::ExtractClass);
    assert!(has_large_class, "Expected large class recommendation");
}

#[tokio::test]
async fn test_refactoring_extractor_produces_features() {
    use crate::core::config::ValknutConfig;
    use crate::core::featureset::{CodeEntity, ExtractionContext};

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("long_refactor.py");

    let mut content = String::from("def long_function():\n");
    for i in 0..70 {
        content.push_str(&format!("    value = {}\n", i));
    }
    tokio::fs::write(&file_path, &content).await.unwrap();

    let entity = CodeEntity::new(
        "entity::long_function",
        "function",
        "long_function",
        file_path.to_string_lossy(),
    )
    .with_line_range(1, content.lines().count())
    .with_source_code(content.clone());

    let mut context = ExtractionContext::new(Arc::new(ValknutConfig::default()), "python");
    context.add_entity(entity.clone());

    let extractor = RefactoringExtractor::default();
    let features = extractor.extract(&entity, &context).await.unwrap();

    let recommendation_count = features
        .get("refactoring_recommendation_count")
        .copied()
        .unwrap_or_default();
    assert!(recommendation_count >= 1.0);

    assert!(
        features
            .get("refactoring_file_score")
            .copied()
            .unwrap_or_default()
            >= 0.0
    );
}
