use base64::{engine::general_purpose, Engine as _};
use handlebars::Handlebars;
use handlebars::{Helper, HelperResult, RenderContext, RenderError};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Serialize a value to JSON for template consumption. Returns `Value::Null` on error.
pub fn safe_json_value<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or_else(|e| {
        eprintln!("Warning: Failed to serialize value to JSON: {}", e);
        Value::Null
    })
}

/// Register all Handlebars helpers used by Valknut reports.
pub fn register_helpers(handlebars: &mut Handlebars<'static>) {
    // Helper: pretty-print JSON objects
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

    // Helper: format numbers using a shorthand format string
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

    // Helper: convert a fraction to a percentage string with optional precision
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

    // Helper: multiply two numeric values
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

    // Helper: capitalize the first letter of a string
    handlebars.register_helper(
        "capitalize",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("capitalize helper requires a string parameter")
                })?;
                let mut chars = value.chars();
                let transformed = if let Some(first) = chars.next() {
                    format!("{}{}", first.to_uppercase(), chars.as_str())
                } else {
                    value.to_string()
                };
                out.write(&transformed)?;
                Ok(())
            },
        ),
    );

    // Helper: replace text within a string
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

    // Helper: subtract two numbers
    register_simple_numeric_helper(handlebars, "subtract", |a, b| a - b);
    // Helper: add two numbers
    register_simple_numeric_helper(handlebars, "add", |a, b| a + b);
    // Helper: greater-than comparison
    register_simple_numeric_helper(handlebars, "gt", |a, b| (a > b) as i32 as f64);

    // Helper: count required tasks in an array (where required == true)
    handlebars.register_helper(
        "count_required",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let array = h
                    .param(0)
                    .and_then(|v| v.value().as_array())
                    .ok_or_else(|| {
                        RenderError::new("count_required helper requires an array parameter")
                    })?;
                let count = array
                    .iter()
                    .filter(|item| {
                        item.get("required")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    })
                    .count();
                out.write(&count.to_string())?;
                Ok(())
            },
        ),
    );

    // Helper: count optional tasks in an array (where required == false or missing)
    handlebars.register_helper(
        "count_optional",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let array = h
                    .param(0)
                    .and_then(|v| v.value().as_array())
                    .ok_or_else(|| {
                        RenderError::new("count_optional helper requires an array parameter")
                    })?;
                let count = array
                    .iter()
                    .filter(|item| {
                        !item
                            .get("required")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    })
                    .count();
                out.write(&count.to_string())?;
                Ok(())
            },
        ),
    );

    // Helper: array length
    handlebars.register_helper(
        "length",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let array = h
                    .param(0)
                    .and_then(|v| v.value().as_array())
                    .ok_or_else(|| RenderError::new("length helper requires an array parameter"))?;
                out.write(&array.len().to_string())?;
                Ok(())
            },
        ),
    );

    // Helper: array emptiness check
    handlebars.register_helper(
        "has_children",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let array = h
                    .param(0)
                    .and_then(|v| v.value().as_array())
                    .ok_or_else(|| {
                        RenderError::new("has_children helper requires an array parameter")
                    })?;
                out.write(&(!array.is_empty()).to_string())?;
                Ok(())
            },
        ),
    );

    // Helper: basename extraction
    handlebars.register_helper(
        "basename",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let path_str = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("basename helper requires a string parameter")
                })?;
                let path = Path::new(path_str);
                let basename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path_str);
                out.write(basename)?;
                Ok(())
            },
        ),
    );

    // Helper: equality comparison
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

    // Helper: extract function name from entity identifiers
    handlebars.register_helper(
        "function_name",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("function_name helper requires a string parameter")
                })?;
                let name = value.rsplit(':').next().unwrap_or(value);
                out.write(name)?;
                Ok(())
            },
        ),
    );

    // Helper: map health score to CSS badge class
    handlebars.register_helper(
        "health_badge_class",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_f64()).ok_or_else(|| {
                    RenderError::new("health_badge_class helper requires a numeric parameter")
                })?;
                let badge_class = if value >= 75.0 {
                    "tree-badge-High"
                } else if value >= 50.0 {
                    "tree-badge-Medium"
                } else {
                    "tree-badge-Low"
                };
                out.write(badge_class)?;
                Ok(())
            },
        ),
    );

    // Helper: determine if a directory path likely belongs to the source tree
    handlebars.register_helper(
        "is_source_directory",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let dir_path = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("is_source_directory helper requires a string parameter")
                })?;
                let is_source = dir_path.starts_with("src/")
                    || dir_path == "src"
                    || dir_path.starts_with("tests/")
                    || dir_path == "tests"
                    || dir_path.starts_with("benches/")
                    || dir_path == "benches"
                    || dir_path.starts_with("examples/")
                    || dir_path == "examples"
                    || dir_path.starts_with("scripts/")
                    || dir_path == "scripts"
                    || dir_path.starts_with("vscode-extension/")
                    || dir_path == "vscode-extension";
                out.write(&is_source.to_string())?;
                Ok(())
            },
        ),
    );

    // Helper: inline CSS file content
    handlebars.register_helper(
        "inline_css",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let file_path = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("inline_css helper requires a file path parameter")
                })?;

                // Try multiple possible locations for the CSS file
                let possible_paths = [
                    format!("themes/{}", file_path),
                    format!("./themes/{}", file_path),
                    format!("templates/{}", file_path),
                    format!("./templates/{}", file_path),
                    file_path.to_string(),
                ];

                for path in &possible_paths {
                    if let Ok(content) = fs::read_to_string(path) {
                        out.write(&content)?;
                        return Ok(());
                    }
                }

                // If no file found, use minimal fallback
                if file_path.contains("sibylline.css") {
                    out.write(super::assets::MINIMAL_SIBYLLINE_CSS)?;
                } else {
                    eprintln!(
                        "Warning: CSS file '{}' not found, using empty content",
                        file_path
                    );
                }

                Ok(())
            },
        ),
    );

    // Helper: inline JavaScript file content
    handlebars.register_helper(
        "inline_js",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                let file_path = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
                    RenderError::new("inline_js helper requires a file path parameter")
                })?;

                // Try multiple possible locations for the JavaScript file
                let possible_paths = [
                    format!("templates/assets/dist/{}", file_path),
                    format!("./templates/assets/dist/{}", file_path),
                    format!("templates/assets/{}", file_path),
                    format!("./templates/assets/{}", file_path),
                    file_path.to_string(),
                ];

                for path in &possible_paths {
                    if let Ok(content) = fs::read_to_string(path) {
                        out.write(&content)?;
                        return Ok(());
                    }
                }

                eprintln!(
                    "Warning: JavaScript file '{}' not found, using empty content",
                    file_path
                );
                Ok(())
            },
        ),
    );

    // Helper: inline logo as data URL
    handlebars.register_helper(
        "logo_data_url",
        Box::new(
            |_h: &Helper,
             _: &Handlebars,
             _: &handlebars::Context,
             _: &mut RenderContext,
             out: &mut dyn handlebars::Output|
             -> HelperResult {
                // Try to find the logo file
                let possible_paths = [
                    "assets/logo.webp",
                    "./assets/logo.webp",
                    "webpage_files/valknut-large.webp",
                    "./webpage_files/valknut-large.webp",
                    ".valknut/webpage_files/valknut-large.webp",
                ];

                for path in &possible_paths {
                    if let Ok(content) = fs::read(path) {
                        if !content.is_empty() {
                            let base64_content = general_purpose::STANDARD.encode(&content);
                            let data_url = format!("data:image/webp;base64,{}", base64_content);
                            out.write(&data_url)?;
                            return Ok(());
                        }
                    }
                }

                // Fallback: Use a simple SVG placeholder
                let svg_placeholder = r#"data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTAwIiBoZWlnaHQ9IjEwMCIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KICA8cmVjdCB3aWR0aD0iMTAwIiBoZWlnaHQ9IjEwMCIgZmlsbD0iIzMzMzMzMyIvPgogIDx0ZXh0IHg9IjUwIiB5PSI1NSIgZm9udC1mYW1pbHk9IkFyaWFsIiBmb250LXNpemU9IjE0IiBmaWxsPSJ3aGl0ZSIgdGV4dC1hbmNob3I9Im1pZGRsZSI+VmFsa251dDwvdGV4dD4KICA8L3N2Zz4="#;
                out.write(svg_placeholder)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use handlebars::Handlebars;
    use serde::ser::{Serialize, Serializer};
    use serde_json::{json, Value};
    use serial_test::serial;
    use std::env;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    fn expect_render_error(
        handlebars: &Handlebars<'static>,
        ctx: &Value,
        template: &str,
        expected_fragment: &str,
    ) {
        let err = handlebars
            .render_template(template, ctx)
            .expect_err("template should error");
        let error_text = err.to_string();
        assert!(
            error_text.contains(expected_fragment),
            "expected error containing '{expected_fragment}', got '{error_text}'"
        );
    }

    #[test]
    fn safe_json_value_returns_null_on_error() {
        struct FailingValue;
        impl Serialize for FailingValue {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                Err(serde::ser::Error::custom("intentional failure"))
            }
        }

        let null_value = safe_json_value(FailingValue);
        assert_eq!(null_value, Value::Null);

        let data_value = safe_json_value(json!({"name": "valknut"}));
        assert_eq!(data_value["name"], "valknut");
    }

    #[test]
    fn template_helpers_render_expected_values() {
        let mut handlebars = Handlebars::new();
        register_helpers(&mut handlebars);

        let ctx = json!({
            "obj": { "name": "valknut" },
            "number": 42.1234,
            "ratio": 0.256,
            "a": 6.0,
            "b": 7.0,
            "word": "valknut",
            "text": "foo baz",
            "num1": 10.0,
            "num2": 4.0,
            "collection": [1,2],
            "path": "src/lib.rs",
            "lhs": 10,
            "rhs": 10,
            "entity": "crate::module::function_name",
            "health_high": 82.5,
            "health_mid": 60.0,
            "health_low": 25.0,
            "dir": "src/core",
            "other_dir": "docs"
        });

        let json_out = handlebars.render_template("{{json obj}}", &ctx).unwrap();
        assert!(json_out.contains("\"name\": \"valknut\""));

        let format_out = handlebars
            .render_template("{{format number \"0.2\"}}", &ctx)
            .unwrap();
        assert_eq!(format_out, "42.12");

        let percentage_out = handlebars
            .render_template("{{percentage ratio \"1\"}}", &ctx)
            .unwrap();
        assert_eq!(percentage_out, "25.6");

        let multiply_out = handlebars
            .render_template("{{multiply a b}}", &ctx)
            .unwrap();
        assert_eq!(multiply_out, "42");

        let capitalize_out = handlebars
            .render_template("{{capitalize word}}", &ctx)
            .unwrap();
        assert_eq!(capitalize_out, "Valknut");

        let replace_out = handlebars
            .render_template("{{replace text \"foo\" \"bar\"}}", &ctx)
            .unwrap();
        assert_eq!(replace_out, "bar baz");

        let subtract_out = handlebars
            .render_template("{{subtract num1 num2}}", &ctx)
            .unwrap();
        assert_eq!(subtract_out, "6");

        let add_out = handlebars
            .render_template("{{add num1 num2}}", &ctx)
            .unwrap();
        assert_eq!(add_out, "14");

        let gt_out = handlebars
            .render_template("{{gt num1 num2}}", &ctx)
            .unwrap();
        assert_eq!(gt_out, "1");

        let length_out = handlebars
            .render_template("{{length collection}}", &ctx)
            .unwrap();
        assert_eq!(length_out, "2");

        let has_children_out = handlebars
            .render_template("{{has_children collection}}", &ctx)
            .unwrap();
        assert_eq!(has_children_out, "true");

        let basename_out = handlebars
            .render_template("{{basename path}}", &ctx)
            .unwrap();
        assert_eq!(basename_out, "lib.rs");

        let eq_out = handlebars.render_template("{{eq lhs rhs}}", &ctx).unwrap();
        assert_eq!(eq_out, "true");

        let fn_name_out = handlebars
            .render_template("{{function_name entity}}", &ctx)
            .unwrap();
        assert_eq!(fn_name_out, "function_name");

        let high_badge = handlebars
            .render_template("{{health_badge_class health_high}}", &ctx)
            .unwrap();
        assert_eq!(high_badge, "tree-badge-High");
        let mid_badge = handlebars
            .render_template("{{health_badge_class health_mid}}", &ctx)
            .unwrap();
        assert_eq!(mid_badge, "tree-badge-Medium");
        let low_badge = handlebars
            .render_template("{{health_badge_class health_low}}", &ctx)
            .unwrap();
        assert_eq!(low_badge, "tree-badge-Low");

        let is_source_out = handlebars
            .render_template("{{is_source_directory dir}}", &ctx)
            .unwrap();
        assert_eq!(is_source_out, "true");
        let non_source_out = handlebars
            .render_template("{{is_source_directory other_dir}}", &ctx)
            .unwrap();
        assert_eq!(non_source_out, "false");
    }

    #[test]
    fn template_helpers_cover_defaults_and_error_paths() {
        let mut handlebars = Handlebars::new();
        register_helpers(&mut handlebars);

        let ctx = json!({
            "ratio": 0.256,
            "word": "",
            "text": "foo baz",
            "number": 42.1234,
            "num1": 10.0
        });

        // Cover default percentage formatting (no precision argument)
        let default_percentage = handlebars
            .render_template("{{percentage ratio}}", &ctx)
            .unwrap();
        assert_eq!(default_percentage, "26");

        // Cover capitalize branch when string is empty
        let empty_capitalized = handlebars
            .render_template("{{capitalize word}}", &ctx)
            .unwrap();
        assert!(empty_capitalized.is_empty());

        expect_render_error(
            &handlebars,
            &ctx,
            "{{format}}",
            "format helper requires a numeric parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{percentage}}",
            "percentage helper requires a numeric parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{multiply}}",
            "multiply helper requires a numeric parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{multiply number}}",
            "multiply helper requires a second numeric parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{capitalize}}",
            "capitalize helper requires a string parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{replace}}",
            "replace helper requires a string parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{replace text}}",
            "replace helper requires a search string",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{replace text \"foo\"}}",
            "replace helper requires a replacement string",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{length}}",
            "length helper requires an array parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{has_children}}",
            "has_children helper requires an array parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{basename}}",
            "basename helper requires a string parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{function_name}}",
            "function_name helper requires a string parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{health_badge_class}}",
            "health_badge_class helper requires a numeric parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{is_source_directory}}",
            "is_source_directory helper requires a string parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{inline_css}}",
            "inline_css helper requires a file path parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{inline_js}}",
            "inline_js helper requires a file path parameter",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{subtract}}",
            "subtract helper requires numeric parameters",
        );
        expect_render_error(
            &handlebars,
            &ctx,
            "{{subtract num1}}",
            "subtract helper requires two numeric parameters",
        );

        // Ensure missing CSS asset path falls back to warning branch
        let missing_css = handlebars
            .render_template("{{inline_css \"missing.css\"}}", &ctx)
            .unwrap();
        assert!(missing_css.is_empty());
    }

    struct DirGuard {
        original: PathBuf,
    }

    impl DirGuard {
        fn new<P: AsRef<Path>>(target: P) -> Self {
            let original = env::current_dir().expect("current dir");
            env::set_current_dir(target.as_ref()).expect("set current dir");
            Self { original }
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.original).expect("restore current dir");
        }
    }

    #[serial]
    #[test]
    fn file_helpers_load_assets_and_fallback_gracefully() {
        let temp = tempdir().unwrap();
        let _guard = DirGuard::new(temp.path());

        fs::create_dir_all("themes").unwrap();
        fs::write("themes/custom.css", "body { color: blue; }").unwrap();

        fs::create_dir_all("templates/assets/dist").unwrap();
        fs::write(
            "templates/assets/dist/app.js",
            "console.log('hello valknut');",
        )
        .unwrap();

        fs::create_dir_all("assets").unwrap();
        fs::write("assets/logo.webp", &[0u8, 1, 2, 3]).unwrap();

        let mut handlebars = Handlebars::new();
        register_helpers(&mut handlebars);

        let empty = json!({});

        let css = handlebars
            .render_template("{{inline_css \"custom.css\"}}", &empty)
            .unwrap();
        assert!(css.contains("color: blue"));

        let js = handlebars
            .render_template("{{inline_js \"app.js\"}}", &empty)
            .unwrap();
        assert!(js.contains("hello valknut"));

        let logo = handlebars
            .render_template("{{logo_data_url}}", &empty)
            .unwrap();
        assert!(logo.starts_with("data:image/webp;base64"));

        let fallback_css = handlebars
            .render_template("{{inline_css \"sibylline.css\"}}", &empty)
            .unwrap();
        assert!(fallback_css.contains("font-family"));

        let missing_js = handlebars
            .render_template("{{inline_js \"missing.js\"}}", &empty)
            .unwrap();
        assert!(missing_js.is_empty());

        fs::remove_file("assets/logo.webp").unwrap();
        let fallback_logo = handlebars
            .render_template("{{logo_data_url}}", &empty)
            .unwrap();
        assert!(fallback_logo.contains("data:image/svg+xml;base64"));
    }
}
