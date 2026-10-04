use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;
use std::process::Command;

pub(crate) fn configure_command(command: &mut Command, guard_root: &Path, fixture_label: &str) {
    let guard_directory = guard_root.join("artifact-path-guard");
    std::fs::create_dir(&guard_directory).unwrap_or_else(|error| {
        panic!("create private {fixture_label} artifact path guard: {error}")
    });
    std::fs::set_permissions(&guard_directory, std::fs::Permissions::from_mode(0o700))
        .unwrap_or_else(|error| {
            panic!("secure private {fixture_label} artifact path guard: {error}")
        });
    let metadata = std::fs::metadata(&guard_directory).unwrap_or_else(|error| {
        panic!("inspect private {fixture_label} artifact path guard: {error}")
    });
    let identity = format!("{:016x}:{:016x}", metadata.dev(), metadata.ino());
    command
        .env("FE2O3_ARTIFACT_PATH_GUARD_DIR", &guard_directory)
        .env("FE2O3_ARTIFACT_PATH_GUARD_DIR_IDENTITY", identity);
}

#[allow(dead_code)]
pub(crate) fn rerun_current_test(
    test_name: &str,
    child_marker_env: &str,
    guard_root: &Path,
    fixture_label: &str,
) -> bool {
    if let Some(marker) = std::env::var_os(child_marker_env) {
        assert_eq!(
            marker,
            std::ffi::OsString::from("1"),
            "configured artifact guard child marker must be exactly 1"
        );
        return false;
    }

    let mut command =
        Command::new(std::env::current_exe().expect("current integration test executable"));
    command
        .args(["--exact", test_name, "--nocapture"])
        .env(child_marker_env, "1");
    configure_command(&mut command, guard_root, fixture_label);
    let child = command.output().unwrap_or_else(|error| {
        panic!("run {fixture_label} with configured artifact guard: {error}")
    });
    assert!(
        child.status.success(),
        "configured artifact-path-guard {fixture_label} test failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
    true
}
