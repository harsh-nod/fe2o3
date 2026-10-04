use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub fn fixture_target_dir(name: &str) -> PathBuf {
    let root = std::env::var_os("CARGO_TARGET_DIR");
    target_dir(root.as_deref(), Path::new(env!("CARGO_MANIFEST_DIR")), name)
}

fn target_dir(root: Option<&OsStr>, manifest_dir: &Path, name: &str) -> PathBuf {
    root.map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../target"))
        .join(name)
}

#[test]
fn configured_fixture_targets_retain_the_selected_root() {
    for root in ["/tmp/fe2o3 target", "relative-target"] {
        assert_eq!(
            target_dir(
                Some(OsStr::new(root)),
                Path::new("/source/macros"),
                "fixture"
            ),
            Path::new(root).join("fixture")
        );
    }
}

#[test]
fn unconfigured_fixture_targets_keep_the_workspace_fallback() {
    assert_eq!(
        target_dir(None, Path::new("/source/crates/macros"), "fixture"),
        Path::new("/source/crates/macros/../../target/fixture")
    );
}
