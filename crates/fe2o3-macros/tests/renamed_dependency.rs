use std::path::PathBuf;
use std::process::Command;

#[test]
fn kernel_marker_resolves_renamed_device_dependency() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-device/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/renamed-device-marker-test");

    let output = Command::new(env!("CARGO"))
        .arg("check")
        .arg("--offline")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .output()
        .expect("failed to run cargo check for renamed-device fixture");

    assert!(
        output.status.success(),
        "renamed-device fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_control_flow_sidecar_decodes_canonically() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-device/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/renamed-device-marker-test");

    let output = Command::new(env!("CARGO"))
        .arg("run")
        .arg("--offline")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir)
        .output()
        .expect("failed to run renamed-device control-flow fixture");

    assert!(
        output.status.success(),
        "renamed-device control-flow fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
