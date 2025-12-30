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
