use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use handlebars::Handlebars;

use super::error::ReportError;

pub(super) const FALLBACK_TEMPLATE_NAME: &str = "default_html";
pub(super) const MARKDOWN_TEMPLATE_NAME: &str = "markdown_report";
pub(super) const CSV_TEMPLATE_NAME: &str = "csv_report";
pub(super) const SONAR_TEMPLATE_NAME: &str = "sonar_report";

pub(super) fn register_fallback_template(handlebars: &mut Handlebars<'static>) {
    if let Err(err) = handlebars
        .register_template_string(FALLBACK_TEMPLATE_NAME, include_str!("./default_report.hbs"))
    {
        eprintln!("Failed to register fallback HTML template: {}", err);
    }

    if let Err(err) = handlebars.register_template_string(
        MARKDOWN_TEMPLATE_NAME,
        include_str!("./default_markdown.hbs"),
    ) {
        eprintln!("Failed to register fallback Markdown template: {}", err);
    }

    if let Err(err) =
        handlebars.register_template_string(CSV_TEMPLATE_NAME, include_str!("./default_csv.hbs"))
    {
        eprintln!("Failed to register fallback CSV template: {}", err);
    }

    if let Err(err) = handlebars
        .register_template_string(SONAR_TEMPLATE_NAME, include_str!("./default_sonar.hbs"))
    {
        eprintln!("Failed to register fallback Sonar template: {}", err);
    }
}

/// Process .hbs files in a directory, calling the provided registration function for each.
fn process_hbs_files<F>(dir: &Path, mut register: F) -> Result<(), ReportError>
where
    F: FnMut(&str, String) -> Result<(), ReportError>,
{
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) == Some("hbs") {
            let name = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid template filename")
            })?;
            register(name, fs::read_to_string(&path)?)?;
        }
    }
    Ok(())
}

pub(super) fn load_templates_from_dir(
    handlebars: &mut Handlebars<'static>,
    templates_dir: &Path,
) -> Result<(), ReportError> {
    process_hbs_files(templates_dir, |name, content| {
        handlebars.register_template_string(name, content)?;
        Ok(())
    })?;

    let partials_dir = templates_dir.join("partials");
    if partials_dir.exists() && partials_dir.is_dir() {
        process_hbs_files(&partials_dir, |name, content| {
            handlebars.register_partial(name, content)?;
            Ok(())
        })?;
    }

    Ok(())
}

pub(super) fn detect_templates_dir() -> Option<PathBuf> {
    if let Ok(custom_root) = env::var("VALKNUT_TEMPLATE_ROOT") {
        let path = PathBuf::from(custom_root);
        if path.exists() {
            return Some(path);
        }
    }

    env::current_dir()
        .ok()
        .map(|cwd| cwd.join("templates"))
        .filter(|path| path.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use handlebars::Handlebars;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn load_templates_from_dir_registers_templates_and_partials() {
        let temp = tempdir().unwrap();
        let templates_dir = temp.path();

        std::fs::create_dir_all(templates_dir.join("partials")).unwrap();
        std::fs::write(
            templates_dir.join("custom.hbs"),
            "{{#each items}}{{> item}}{{/each}}",
        )
        .unwrap();
        std::fs::write(
            templates_dir.join("partials").join("item.hbs"),
            "<li>{{this}}</li>",
        )
        .unwrap();

        let mut handlebars = Handlebars::new();
        load_templates_from_dir(&mut handlebars, templates_dir).expect("load templates");

        assert!(handlebars.get_templates().contains_key("custom"));
        let rendered = handlebars
            .render("custom", &json!({ "items": ["one", "two"] }))
            .expect("render custom");
        assert!(rendered.contains("<li>one</li>"));
        assert!(rendered.contains("<li>two</li>"));
    }

    #[cfg(unix)]
    #[test]
    fn load_templates_from_dir_errors_on_invalid_template_filename() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let temp = tempdir().unwrap();
        let invalid_name = OsString::from_vec(vec![0xFF, b'.', b'h', b'b', b's']);
        let invalid_path = temp.path().join(&invalid_name);
        std::fs::write(&invalid_path, "{{this}}").unwrap();

        let mut handlebars = Handlebars::new();
        let err = load_templates_from_dir(&mut handlebars, temp.path()).unwrap_err();
        assert!(
            format!("{}", err).contains("Invalid template filename"),
            "unexpected error: {err:?}"
        );

        // Ensure directory still usable for valid files afterwards
        std::fs::remove_file(&invalid_path).unwrap();
        let valid_name =
            OsString::from_vec(vec![b'v', b'a', b'l', b'i', b'd', b'.', b'h', b'b', b's']);
        let valid_path = temp.path().join(&valid_name);
        std::fs::write(&valid_path, "<p>{{this}}</p>").unwrap();
        load_templates_from_dir(&mut handlebars, temp.path()).expect("reload with valid file");
    }

    #[cfg(unix)]
    #[test]
    fn load_templates_errors_on_invalid_partial_filename() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let temp = tempdir().unwrap();
        let templates_dir = temp.path();
        let partials_dir = templates_dir.join("partials");
        std::fs::create_dir_all(&partials_dir).unwrap();
        let invalid_partial = OsString::from_vec(vec![0xFE, b'.', b'h', b'b', b's']);
        std::fs::write(partials_dir.join(&invalid_partial), "{{this}}").unwrap();

        let mut handlebars = Handlebars::new();
        let err = super::load_templates_from_dir(&mut handlebars, templates_dir).unwrap_err();
        assert!(
            format!("{}", err).contains("Invalid template filename"),
            "unexpected error: {err:?}"
        );
    }
}
