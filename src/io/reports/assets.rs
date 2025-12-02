use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::error::ReportError;

pub(super) const MINIMAL_SIBYLLINE_CSS: &str = include_str!("./minimal_sibylline.css");

fn js_asset_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(custom_root) = env::var("VALKNUT_TEMPLATE_ROOT") {
        let base = PathBuf::from(&custom_root);
        roots.push(base.join("assets/dist"));
        roots.push(base.join("assets/src"));
        roots.push(base.join("assets"));
    }

    roots.push(PathBuf::from("templates/assets/dist"));
    roots.push(PathBuf::from("./templates/assets/dist"));
    roots.push(PathBuf::from("templates/assets/src"));
    roots.push(PathBuf::from("./templates/assets/src"));
    roots.push(PathBuf::from("templates/assets"));
    roots.push(PathBuf::from("./templates/assets"));

    roots
}

fn webpage_asset_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(custom_root) = env::var("VALKNUT_TEMPLATE_ROOT") {
        let base = PathBuf::from(&custom_root);
        roots.push(base.join("assets"));
        roots.push(base);
    }

    roots.push(PathBuf::from("templates/assets"));
    roots.push(PathBuf::from("./templates/assets"));
    roots.push(PathBuf::from("templates"));
    roots.push(PathBuf::from("./templates"));

    roots
}

pub(super) fn copy_theme_css_to_output<P: AsRef<Path>>(output_dir: P) -> Result<(), ReportError> {
    let output_dir = output_dir.as_ref();

    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    let possible_theme_paths = [
        Path::new("themes/sibylline.css"),
        Path::new("./themes/sibylline.css"),
    ];

    for theme_path in possible_theme_paths {
        if theme_path.exists() {
            fs::copy(theme_path, output_dir.join("sibylline.css"))?;
            return Ok(());
        }
    }

    fs::write(output_dir.join("sibylline.css"), MINIMAL_SIBYLLINE_CSS)?;
    Ok(())
}

pub(super) fn copy_js_assets_to_output<P: AsRef<Path>>(output_dir: P) -> Result<(), ReportError> {
    let output_dir = output_dir.as_ref();

    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    let js_files = [
        ("react-tree-bundle.js", "react-tree-bundle.js"),
        ("react-tree-bundle.debug.js", "react-tree-bundle.debug.js"),
        ("tree-fallback.js", "tree-fallback.js"),
    ];
    let search_roots = js_asset_roots();

    for (src, dest) in js_files {
        let mut copied = false;
        for root in &search_roots {
            let asset_path = root.join(src);
            if asset_path.exists() {
                fs::copy(asset_path, output_dir.join(dest))?;
                copied = true;
                break;
            }
        }

        if !copied {
            eprintln!(
                "Warning: JavaScript asset {} not found; the interactive tree may not render",
                src
            );
        }
    }

    Ok(())
}

pub fn copy_webpage_assets_to_output<P: AsRef<Path>>(output_dir: P) -> Result<(), ReportError> {
    let output_dir = output_dir.as_ref();

    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    let webpage_files_dir = output_dir.join("webpage_files");
    if !webpage_files_dir.exists() {
        fs::create_dir_all(&webpage_files_dir)?;
    }

    let assets = [
        ("webpage_files/valknut-large.webp", "webpage_files"),
        ("webpage_files/three.min.js", "webpage_files"),
        ("webpage_files/trefoil-animation.js", "webpage_files"),
    ];
    let search_roots = webpage_asset_roots();

    for (relative_source, target_dir) in assets {
        let mut copied = false;
        for root in &search_roots {
            let source_path = root.join(relative_source);
            if source_path.exists() {
                let destination = output_dir.join(target_dir).join(
                    Path::new(relative_source)
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new("asset")),
                );
                if let Some(parent) = destination.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                fs::copy(source_path, destination)?;
                copied = true;
                break;
            }
        }

        if !copied {
            eprintln!("Warning: asset {} not found", relative_source);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

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

    struct EnvVarGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvVarGuard {
        fn set<S: Into<String>>(key: &'static str, value: S) -> Self {
            let original = env::var(key).ok();
            env::set_var(key, value.into());
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(ref value) = self.original {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    #[serial]
    #[test]
    fn copy_theme_css_prefers_existing_theme_and_falls_back() {
        let temp = tempdir().unwrap();
        let _guard = DirGuard::new(temp.path());

        fs::create_dir_all("themes").unwrap();
        fs::write("themes/sibylline.css", "/* custom theme */").unwrap();

        let output_dir = Path::new("out/theme");
        copy_theme_css_to_output(output_dir).expect("copy theme");
        let copied = fs::read_to_string(output_dir.join("sibylline.css")).unwrap();
        assert_eq!(copied, "/* custom theme */");

        // Remove the theme file to exercise the embedded minimal stylesheet path
        fs::remove_file("themes/sibylline.css").unwrap();
        fs::remove_dir_all(output_dir).unwrap();

        copy_theme_css_to_output(output_dir).expect("copy fallback theme");
        let fallback = fs::read_to_string(output_dir.join("sibylline.css")).unwrap();
        assert_eq!(fallback, MINIMAL_SIBYLLINE_CSS);
    }

    #[serial]
    #[test]
    fn copy_js_assets_copies_available_files() {
        let temp = tempdir().unwrap();
        let _guard = DirGuard::new(temp.path());

        let template_root = temp.path().join("templates");
        let _env_guard = EnvVarGuard::set(
            "VALKNUT_TEMPLATE_ROOT",
            template_root.to_string_lossy().into_owned(),
        );
        fs::create_dir_all(template_root.join("assets/dist")).unwrap();
        fs::write(
            template_root.join("assets/dist/react-tree-bundle.js"),
            "console.log('bundle');",
        )
        .unwrap();

        let output_dir = Path::new("out/js");
        copy_js_assets_to_output(output_dir).expect("copy js assets");

        let copied = fs::read_to_string(output_dir.join("react-tree-bundle.js")).unwrap();
        assert!(copied.contains("bundle"));

        // Files that don't exist should not cause an error, but no file should appear either
        assert!(
            !output_dir.join("react-tree-bundle.debug.js").exists(),
            "missing assets should not be created"
        );
    }

    #[serial]
    #[test]
    fn copy_webpage_assets_creates_directories_and_copies_present_assets() {
        let temp = tempdir().unwrap();
        let _guard = DirGuard::new(temp.path());

        let template_root = temp.path().join("templates");
        let _env_guard = EnvVarGuard::set(
            "VALKNUT_TEMPLATE_ROOT",
            template_root.to_string_lossy().into_owned(),
        );
        fs::create_dir_all(template_root.join("assets/webpage_files")).unwrap();
        fs::write(
            template_root.join("assets/webpage_files/valknut-large.webp"),
            vec![1, 2, 3, 4],
        )
        .unwrap();

        let output_dir = Path::new("out/site");
        copy_webpage_assets_to_output(output_dir).expect("copy webpage assets");

        assert!(
            output_dir.join("webpage_files/valknut-large.webp").exists(),
            "existing asset should be copied"
        );
        assert!(
            output_dir.join("webpage_files/three.min.js").exists() == false,
            "missing assets should be skipped without error"
        );
    }
}
