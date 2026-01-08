use base64::{engine::general_purpose, Engine as _};
use handlebars::Handlebars;
use handlebars::{Helper, HelperResult, RenderContext, RenderError};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;

/// Directories considered part of the source tree
const SOURCE_DIRECTORIES: &[&str] = &[
    "src/", "src", "tests/", "tests", "benches/", "benches",
    "examples/", "examples", "scripts/", "scripts",
    "vscode-extension/", "vscode-extension",
];

/// Path prefixes for CSS file lookup
const CSS_PREFIXES: &[&str] = &["themes/", "./themes/", "templates/", "./templates/", ""];

/// Path prefixes for JavaScript file lookup
const JS_PREFIXES: &[&str] = &[
    "templates/assets/dist/", "./templates/assets/dist/",
    "templates/assets/", "./templates/assets/", "",
];

/// Serialize a value to JSON for template consumption. Returns `Value::Null` on error.
pub fn safe_json_value<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to serialize value to JSON: {}", e);
        Value::Null
    })
}

/// Register all Handlebars helpers used by Valknut reports.
pub fn register_helpers(handlebars: &mut Handlebars<'static>) {
    register_json_helper(handlebars);
    register_format_helper(handlebars);
    register_percentage_helper(handlebars);
    register_multiply_helper(handlebars);

    // Helper: capitalize the first letter of a string
    register_string_transform_helper(handlebars, "capitalize", |value| {
        let mut chars = value.chars();
        if let Some(first) = chars.next() {
            format!("{}{}", first.to_uppercase(), chars.as_str())
        } else {
            value.to_string()
        }
    });

    register_replace_helper(handlebars);

    // Helper: subtract two numbers
    register_simple_numeric_helper(handlebars, "subtract", |a, b| a - b);
    // Helper: add two numbers
    register_simple_numeric_helper(handlebars, "add", |a, b| a + b);
    // Helper: greater-than comparison
    register_simple_numeric_helper(handlebars, "gt", |a, b| (a > b) as i32 as f64);

    // Helper: count required tasks in an array (where required == true)
    register_array_helper(handlebars, "count_required", |array| {
        array
            .iter()
            .filter(|item| {
                item.get("required")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .count()
            .to_string()
    });

    // Helper: count optional tasks in an array (where required == false or missing)
    register_array_helper(handlebars, "count_optional", |array| {
        array
            .iter()
            .filter(|item| {
                !item
                    .get("required")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .count()
            .to_string()
    });

    // Helper: array length
    register_array_helper(handlebars, "length", |array| array.len().to_string());

    // Helper: array emptiness check
    register_array_helper(handlebars, "has_children", |array| {
        (!array.is_empty()).to_string()
    });

    // Helper: basename extraction
    register_string_transform_helper(handlebars, "basename", |path_str| {
        Path::new(path_str)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path_str)
            .to_string()
    });

    register_eq_helper(handlebars);

    // Helper: extract function name from entity identifiers
    register_string_transform_helper(handlebars, "function_name", |value| {
        value.rsplit(':').next().unwrap_or(value).to_string()
    });

    // Helper: map health score to CSS badge class
    register_numeric_threshold_helper(handlebars, "health_badge_class", |value| {
        if value >= 75.0 {
            "tree-badge-High"
        } else if value >= 50.0 {
            "tree-badge-Medium"
        } else {
            "tree-badge-Low"
        }
    });

    // Helper: determine if a directory path likely belongs to the source tree
    register_string_transform_helper(handlebars, "is_source_directory", |dir_path| {
        let is_source = SOURCE_DIRECTORIES.iter()
            .any(|d| dir_path.starts_with(d) || dir_path == *d);
        is_source.to_string()
    });

    // Helper: inline CSS file content
    register_inline_file_helper(handlebars, InlineFileConfig {
        name: "inline_css",
        prefixes: CSS_PREFIXES,
        fallback: Some(("sibylline.css", super::assets::MINIMAL_SIBYLLINE_CSS)),
        file_type: "CSS",
    });

    // Helper: inline JavaScript file content
    register_inline_file_helper(handlebars, InlineFileConfig {
        name: "inline_js",
        prefixes: JS_PREFIXES,
        fallback: None,
        file_type: "JavaScript",
    });

    register_logo_data_url_helper(handlebars);
}

/// Register the JSON pretty-print helper
fn register_json_helper(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper(
        "json",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h
                    .param(0)
                    .map(|v| v.value())
                    .ok_or_else(|| RenderError::new("json helper requires a parameter"))?;
                let json_str = serde_json::to_string_pretty(value)
                    .map_err(|e| RenderError::new(&format!("JSON serialization error: {}", e)))?;
                out.write(&json_str)?;
                Ok(())
            },
        ),
    );
}

/// Register the format number helper
fn register_format_helper(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper(
        "format",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new("format helper requires a numeric parameter")
                })?;
                let format_str = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("0.1");
                let rendered = match format_str {
                    "0.0" => format!("{:.0}", value),
                    "0.2" => format!("{:.2}", value),
                    _ => format!("{:.1}", value),
                };
                out.write(&rendered)?;
                Ok(())
            },
        ),
    );
}

/// Register the percentage helper
fn register_percentage_helper(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper(
        "percentage",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new("percentage helper requires a numeric parameter")
                })?;
                let decimals = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("0");
                let percentage = value * 100.0;
                let rendered = match decimals {
                    "1" => format!("{:.1}", percentage),
                    "2" => format!("{:.2}", percentage),
                    _ => format!("{:.0}", percentage),
                };
                out.write(&rendered)?;
                Ok(())
            },
        ),
    );
}

/// Register the multiply helper
fn register_multiply_helper(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper(
        "multiply",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new("multiply helper requires a numeric parameter")
                })?;
                let multiplier = h.param(1).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new("multiply helper requires a second numeric parameter")
                })?;
                out.write(&format!("{:.0}", value * multiplier))?;
                Ok(())
            },
        ),
    );
}

/// Register the replace helper
fn register_replace_helper(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper(
        "replace",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("replace helper requires a string parameter")
                })?;
                let search = h
                    .param(1)
                    .and_then(|v| v.value().as_str())
                    .ok_or_else(|| RenderError::new("replace helper requires a search string"))?;
                let replacement = h.param(2).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("replace helper requires a replacement string")
                })?;
                out.write(&value.replace(search, replacement))?;
                Ok(())
            },
        ),
    );
}

/// Register the equality comparison helper
fn register_eq_helper(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper(
        "eq",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let a = h
                    .param(0)
                    .map(|v| v.value().clone())
                    .ok_or_else(|| RenderError::new("eq helper requires two parameters"))?;
                let b = h
                    .param(1)
                    .map(|v| v.value().clone())
                    .ok_or_else(|| RenderError::new("eq helper requires two parameters"))?;
                out.write(&(a == b).to_string())?;
                Ok(())
            },
        ),
    );
}

/// Logo paths to search for the data URL helper (relative to CWD or template root)
const LOGO_RELATIVE_PATHS: &[&str] = &[
    "assets/logo.webp",
    "webpage_files/valknut-large.webp",
    "assets/webpage_files/valknut-large.webp",
    ".valknut/webpage_files/valknut-large.webp",
];

/// SVG placeholder for when logo is not found
const LOGO_SVG_PLACEHOLDER: &str = r#"data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTAwIiBoZWlnaHQ9IjEwMCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KICA8cmVjdCB3aWR0aD0iMTAwIiBoZWlnaHQ9IjEwMCIgZmlsbD0iIzMzMzMzMyIvPgogIDx0ZXh0IHg9IjUwIiB5PSI1NSIgZm9udC1mYW1pbHk9IkFyaWFsIiBmb250LXNpemU9IjE0IiBmaWxsPSJ3aGl0ZSIgdGV4dC1hbmNob3I9Im1pZGRsZSI+VmFsa251dDwvdGV4dD4KICA8L3N2Zz4="#;

/// Register the logo data URL helper
fn register_logo_data_url_helper(handlebars: &mut Handlebars<'static>) {
    handlebars.register_helper(
        "logo_data_url",
        Box::new(
            |_h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                // Build list of paths to check: template root paths first, then relative paths
                let mut paths_to_check: Vec<std::path::PathBuf> = Vec::new();

                // Check VALKNUT_TEMPLATE_ROOT first (used in container deployments)
                if let Ok(template_root) = std::env::var("VALKNUT_TEMPLATE_ROOT") {
                    let root = std::path::Path::new(&template_root);
                    for rel_path in LOGO_RELATIVE_PATHS {
                        paths_to_check.push(root.join(rel_path));
                    }
                }

                // Also check relative to CWD
                for rel_path in LOGO_RELATIVE_PATHS {
                    paths_to_check.push(std::path::PathBuf::from(rel_path));
                }

                for path in paths_to_check {
                    if let Ok(content) = fs::read(&path) {
                        if !content.is_empty() {
                            let base64_content = general_purpose::STANDARD.encode(&content);
                            let data_url = format!("data:image/webp;base64,{}", base64_content);
                            out.write(&data_url)?;
                            return Ok(());
                        }
                    }
                }
                out.write(LOGO_SVG_PLACEHOLDER)?;
                Ok(())
            },
        ),
    );
}

fn register_simple_numeric_helper<F>(handlebars: &mut Handlebars<'static>, name: &str, op: F)
where
    F: Fn(f64, f64) -> f64 + Send + Sync + 'static,
{
    let helper_name = name.to_string();
    handlebars.register_helper(
        name,
        Box::new(
            move |h: &Helper,
                  _: &Handlebars,
                  _: &handlebars::Context,
                  _: &mut RenderContext,
                  out: &mut dyn handlebars::Output|
                  -> HelperResult {
                let a = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new(&format!(
                        "{} helper requires numeric parameters",
                        helper_name
                    ))
                })?;
                let b = h.param(1).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new(&format!(
                        "{} helper requires two numeric parameters",
                        helper_name
                    ))
                })?;
                let value = op(a, b);
                out.write(&value.to_string())?;
                Ok(())
            },
        ),
    );
}

/// Register a helper that transforms a single string parameter.
fn register_string_transform_helper<F>(handlebars: &mut Handlebars<'static>, name: &str, transform: F)
where
    F: Fn(&str) -> String + Send + Sync + 'static,
{
    let helper_name = name.to_string();
    handlebars.register_helper(
        name,
        Box::new(
            move |h: &Helper,
                  _: &Handlebars,
                  _: &handlebars::Context,
                  _: &mut RenderContext,
                  out: &mut dyn handlebars::Output|
                  -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new(&format!("{} helper requires a string parameter", helper_name))
                })?;
                out.write(&transform(value))?;
                Ok(())
            },
        ),
    );
}

/// Register a helper that computes a result from an array parameter.
fn register_array_helper<F>(handlebars: &mut Handlebars<'static>, name: &str, compute: F)
where
    F: Fn(&[Value]) -> String + Send + Sync + 'static,
{
    let helper_name = name.to_string();
    handlebars.register_helper(
        name,
        Box::new(
            move |h: &Helper,
                  _: &Handlebars,
                  _: &handlebars::Context,
                  _: &mut RenderContext,
                  out: &mut dyn handlebars::Output|
                  -> HelperResult {
                let array = h
                    .param(0)
                    .and_then(|v| v.value().as_array())
                    .ok_or_else(|| {
                        RenderError::new(&format!("{} helper requires an array parameter", helper_name))
                    })?;
                out.write(&compute(array))?;
                Ok(())
            },
        ),
    );
}

/// Register a helper that maps a numeric value to a string via thresholds.
fn register_numeric_threshold_helper<F>(handlebars: &mut Handlebars<'static>, name: &str, classify: F)
where
    F: Fn(f64) -> &'static str + Send + Sync + 'static,
{
    let helper_name = name.to_string();
    handlebars.register_helper(
        name,
        Box::new(
            move |h: &Helper,
                  _: &Handlebars,
                  _: &handlebars::Context,
                  _: &mut RenderContext,
                  out: &mut dyn handlebars::Output|
                  -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new(&format!("{} helper requires a numeric parameter", helper_name))
                })?;
                out.write(classify(value))?;
                Ok(())
            },
        ),
    );
}

/// Configuration for inline file helper registration
struct InlineFileConfig {
    name: &'static str,
    prefixes: &'static [&'static str],
    fallback: Option<(&'static str, &'static str)>, // (pattern, content)
    file_type: &'static str,
}

/// Register a helper that inlines file content from disk, trying multiple paths.
fn register_inline_file_helper(handlebars: &mut Handlebars<'static>, config: InlineFileConfig) {
    let helper_name = config.name.to_string();
    let prefixes: Vec<String> = config.prefixes.iter().map(|s| s.to_string()).collect();
    let fallback_pattern = config.fallback.map(|(p, _)| p.to_string());
    let fallback_content = config.fallback.map(|(_, c)| c);
    let file_type = config.file_type.to_string();

    handlebars.register_helper(
        config.name,
        Box::new(
            move |h: &Helper,
                  _: &Handlebars,
                  _: &handlebars::Context,
                  _: &mut RenderContext,
                  out: &mut dyn handlebars::Output|
                  -> HelperResult {
                let file_path = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new(&format!("{} helper requires a file path parameter", helper_name))
                })?;

                // Build list of paths to try: VALKNUT_TEMPLATE_ROOT first, then CWD-relative
                let mut paths_to_try: Vec<String> = Vec::new();

                // Check VALKNUT_TEMPLATE_ROOT first (used in container deployments)
                if let Ok(template_root) = std::env::var("VALKNUT_TEMPLATE_ROOT") {
                    for prefix in &prefixes {
                        let full_path = if prefix.is_empty() {
                            format!("{}/{}", template_root, file_path)
                        } else {
                            format!("{}/{}{}", template_root, prefix, file_path)
                        };
                        paths_to_try.push(full_path);
                    }
                }

                // Also try CWD-relative paths
                for prefix in &prefixes {
                    let full_path = if prefix.is_empty() {
                        file_path.to_string()
                    } else {
                        format!("{}{}", prefix, file_path)
                    };
                    paths_to_try.push(full_path);
                }

                // Try each path location
                for full_path in &paths_to_try {
                    if let Ok(content) = fs::read_to_string(full_path) {
                        out.write(&content)?;
                        return Ok(());
                    }
                }

                // Handle fallback
                if let (Some(ref pattern), Some(content)) = (&fallback_pattern, fallback_content) {
                    if file_path.contains(pattern) {
                        out.write(content)?;
                        return Ok(());
                    }
                }
                eprintln!(
                    "Warning: {} file '{}' not found, using empty content",
                    file_type, file_path
                );
                Ok(())
            },
        ),
    );
}
