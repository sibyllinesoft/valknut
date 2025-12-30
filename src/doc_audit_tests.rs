use super::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_relative_path() {
    let root = PathBuf::from("/tmp/project");
    let file = PathBuf::from("/tmp/project/src/lib.rs");
    assert_eq!(relative_path(&file, &root), PathBuf::from("src/lib.rs"));
}

#[test]
fn test_is_incomplete_doc_empty() {
    assert!(is_incomplete_doc(""));
}

#[test]
fn test_is_incomplete_doc_todo() {
    assert!(is_incomplete_doc("TODO: fill in"));
}

#[test]
fn test_is_incomplete_doc_ok() {
    assert!(!is_incomplete_doc("Describe behavior"));
}

#[test]
fn audit_reports_python_doc_gap() -> Result<()> {
    let dir = tempdir()?;
    let root = dir.path().to_path_buf();
    let file_path = root.join("sample.py");

    fs::write(
        &file_path,
        r#"def important_function():
    return 42
"#,
    )?;

    let mut config = DocAuditConfig::new(root);
    config.complexity_threshold = usize::MAX; // avoid README enforcement noise

    let result = run_audit(&config)?;
    let issue = result
        .documentation_issues
        .iter()
        .find(|issue| issue.path.ends_with("sample.py"));
    assert!(issue.is_some(), "expected missing docstring to be reported");
    Ok(())
}

#[test]
fn audit_reports_missing_readme_for_complex_directory() -> Result<()> {
    let dir = tempdir()?;
    let root = dir.path();
    let heavy = root.join("complex");
    fs::create_dir_all(&heavy)?;
    for idx in 0..3 {
        fs::write(heavy.join(format!("file{idx}.rs")), "pub fn f() {}")?;
    }
    let nested = heavy.join("inner");
    fs::create_dir_all(&nested)?;
    fs::write(nested.join("lib.rs"), "pub fn inner() {}")?;

    let mut config = DocAuditConfig::new(root.to_path_buf());
    config.complexity_threshold = 2;

    let result = run_audit(&config)?;
    assert!(result
        .missing_readmes
        .iter()
        .any(|issue| issue.path == PathBuf::from("complex")));
    Ok(())
}

#[test]
fn audit_reports_stale_readme_after_commits() -> Result<()> {
    let dir = tempdir()?;
    let root = dir.path();
    let repo = Repository::init(root)?;

    // Initial README commit
    fs::write(root.join("README.md"), "# Project\n")?;
    stage_and_commit(&repo, &["README.md"], "initial");

    // Modify project contents in subsequent commits
    fs::create_dir_all(root.join("src"))?;
    fs::write(root.join("src/lib.rs"), "pub fn lib() {}")?;
    stage_and_commit(&repo, &["src/lib.rs"], "add lib");

    let mut config = DocAuditConfig::new(root.to_path_buf());
    config.complexity_threshold = 0;
    config.max_readme_commits = 0; // treat any subsequent change as stale

    let result = run_audit(&config)?;
    assert!(result
        .stale_readmes
        .iter()
        .any(|issue| issue.path == PathBuf::from("README.md")));
    Ok(())
}

#[test]
fn render_helpers_format_output() -> Result<()> {
    let sample = AuditResult {
        documentation_issues: vec![DocIssue {
            category: "undocumented_python".into(),
            path: PathBuf::from("main.py"),
            line: Some(3),
            symbol: Some("main".into()),
            detail: "Function 'main' is missing a docstring".into(),
        }],
        missing_readmes: vec![DocIssue {
            category: "missing_readme".into(),
            path: PathBuf::from("services"),
            line: None,
            symbol: None,
            detail: "Directory exceeds complexity threshold (12 items) without README".into(),
        }],
        stale_readmes: vec![DocIssue {
            category: "stale_readme".into(),
            path: PathBuf::from("README.md"),
            line: None,
            symbol: None,
            detail: "5 commits touched '.' since README update on 2024-01-01T00:00:00+00:00"
                .into(),
        }],
    };

    let text = render_text(&sample);
    assert!(text.contains("Documentation gaps"));
    assert!(text.contains("Missing READMEs"));
    assert!(text.contains("Stale READMEs"));

    let json = render_json(&sample)?;
    let parsed: serde_json::Value = serde_json::from_str(&json)?;
    assert_eq!(parsed["documentation_issues"].as_array().unwrap().len(), 1);
    Ok(())
}

#[test]
fn compute_complexities_counts_files_and_subdirectories() {
    let root = PathBuf::from("/tmp/project");
    let child = root.join("child");

    let mut dir_info = HashMap::new();
    dir_info.insert(
        child.clone(),
        DirectoryInfo {
            files: vec![child.join("lib.rs")],
            subdirs: Vec::new(),
        },
    );
    dir_info.insert(
        root.clone(),
        DirectoryInfo {
            files: vec![root.join("main.py")],
            subdirs: vec![child.clone()],
        },
    );

    let complexities = compute_complexities(&dir_info);
    assert_eq!(complexities.get(&child), Some(&1));
    assert_eq!(complexities.get(&root), Some(&3));
}

#[test]
fn detect_missing_readmes_skips_directories_with_existing_docs() -> Result<()> {
    let temp = tempdir()?;
    let root = temp.path();
    let component = root.join("services");
    fs::create_dir_all(&component)?;
    fs::write(component.join("README.md"), "# docs")?;

    let mut complexities = HashMap::new();
    complexities.insert(component.clone(), DEFAULT_COMPLEXITY_THRESHOLD + 5);

    let mut config = DocAuditConfig::new(root.to_path_buf());
    config.complexity_threshold = 1;

    let (missing, index) = detect_missing_readmes(&complexities, &config);
    assert!(missing.is_empty(), "expected no missing README issues");
    let readme_path = component.join("README.md");
    assert_eq!(index.get(&readme_path), Some(&component));
    Ok(())
}

#[test]
fn should_ignore_file_respects_suffix_configuration() {
    let root = PathBuf::from("/tmp/project");
    let mut config = DocAuditConfig::new(root.clone());
    config.ignore_suffixes.insert(".ignore-me".into());

    let globset = build_ignore_globset(&config.ignore_globs).unwrap();
    let ignored = should_ignore_file(&root.join("bundle.ignore-me"), &config, &globset);
    let tracked = should_ignore_file(&root.join("lib.rs"), &config, &globset);

    assert!(ignored);
    assert!(!tracked);
}

#[test]
fn should_ignore_file_respects_glob_configuration() {
    let root = PathBuf::from("/tmp/project");
    let mut config = DocAuditConfig::new(root.clone());
    config.ignore_globs = vec!["**/tests/**".into(), "**/*_test.rs".into()];

    let globset = build_ignore_globset(&config.ignore_globs).unwrap();
    let ignored = should_ignore_file(&root.join("tests/utils.rs"), &config, &globset);
    let also_ignored = should_ignore_file(&root.join("svc/api_test.rs"), &config, &globset);
    let tracked = should_ignore_file(&root.join("src/lib.rs"), &config, &globset);

    assert!(ignored);
    assert!(also_ignored);
    assert!(!tracked);
}

#[test]
fn python_scanner_detects_missing_and_incomplete_docstrings() {
    let root = PathBuf::from("/tmp/project");
    let path = root.join("analysis.py");
    let source = r#"
class Service:
    def ok(self):
        \"\"\"Performs the operation\"\"\"
        return 1

    def needs_docs(self):
        \"\"\"TODO: describe\"\"\"
        return 2

async def helper():
    return 3
"#;

    let issues = python::scan_python(source, &path, &root);
    let symbols: Vec<_> = issues
        .iter()
        .map(|issue| issue.symbol.clone().unwrap_or_default())
        .collect();

    assert!(
        symbols.iter().any(|symbol| symbol == "Service.needs_docs"),
        "expected incomplete docstring to be reported"
    );
    assert!(
        symbols.iter().any(|symbol| symbol == "helper"),
        "expected missing docstring for helper"
    );
}

#[test]
fn rust_scanner_flags_undocumented_items() {
    let root = PathBuf::from("/tmp/project");
    let path = root.join("lib.rs");
    let source = r#"
mod utilities {
    pub fn helper() {}
}

/// TODO: finish docs
impl utilities::Helper {
    pub fn build() {}
}

pub struct Widget;

fn needs_docs() {}
"#;

    let issues = rust::scan_rust(source, &path, &root);
    let mut categories: Vec<_> = issues.iter().map(|issue| issue.category.as_str()).collect();
    categories.sort();

    assert!(
        categories.contains(&"undocumented_rust_module"),
        "module without docs should be reported"
    );
    assert!(
        categories.contains(&"undocumented_rust_fn"),
        "functions without docs should be reported"
    );
    assert!(
        categories.contains(&"undocumented_rust_impl"),
        "impl blocks with incomplete docs should be reported"
    );
    assert!(
        categories.contains(&"undocumented_rust_item"),
        "structs without docs should be reported"
    );
}

#[test]
fn typescript_scanner_handles_functions_classes_and_arrows() {
    let root = PathBuf::from("/tmp/project");
    let path = root.join("metrics.ts");
    let source = r#"
/**
 * TODO: doc pending
 */
function summarized(): number {
    return 0;
}

class Widget {
    method() {}
}

const compute = (value: number) => {
    return value * 2;
};
"#;

    let issues = typescript::scan_typescript(source, &path, &root);
    let categories: HashSet<_> = issues.iter().map(|issue| issue.category.as_str()).collect();

    assert!(categories.contains("undocumented_ts_function"));
    assert!(categories.contains("undocumented_ts_class"));
    assert!(categories.contains("undocumented_ts_arrow"));
}

fn stage_and_commit(repo: &Repository, paths: &[&str], message: &str) {
    let mut index = repo.index().expect("index");
    for path in paths {
        index.add_path(Path::new(path)).expect("add path");
    }
    index.write().expect("write index");
    let tree_id = index.write_tree().expect("write tree");
    let tree = repo.find_tree(tree_id).expect("find tree");
    let sig = git2::Signature::now("Test", "test@example.com").expect("signature");

    let parents: Vec<git2::Commit> = repo
        .head()
        .ok()
        .and_then(|reference| reference.peel_to_commit().ok())
        .into_iter()
        .collect();

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .expect("commit");
}
