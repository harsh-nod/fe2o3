use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn cargo_check(manifest: &Path, target_dir: &Path, bin: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command
        .arg("check")
        .arg("--offline")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--target-dir")
        .arg(target_dir);
    if let Some(bin) = bin {
        command.arg("--bin").arg(bin);
    }

    command.output().expect("failed to run cargo check fixture")
}

#[test]
fn typed_kernel_resolves_renamed_host_dependency() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/renamed-typed-host/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/renamed-typed-host-test");
    let output = cargo_check(&manifest, &target_dir, None);

    assert!(
        output.status.success(),
        "renamed typed-host fixture failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn typed_kernel_compile_fail_diagnostics_are_stable() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = manifest_dir.join("tests/fixtures/typed-invalid/Cargo.toml");
    let target_dir = manifest_dir.join("../../target/typed-kernel-invalid-test");
    let cases: &[(&str, &[&str])] = &[
        (
            "invalid_attribute",
            &["#[kernel] accepts only #[kernel] or #[kernel(typed)]"],
        ),
        (
            "invalid_signatures",
            &[
                "#[kernel(typed)] requires a public kernel function",
                "#[kernel(typed)] requires a safe kernel function",
                "#[kernel(typed)] does not support generic kernel functions",
                "#[kernel(typed)] requires the unit return type",
                "#[kernel(typed)] requires `pub fn(&[f32], &[f32], DisjointSlice<f32>)`",
                "#[kernel(typed)] argument 1 must have exact type `&[f32]`",
                "#[kernel(typed)] argument 2 must have exact type `&[f32]`",
                "#[kernel(typed)] argument 3 must have exact type `DisjointSlice<f32>`",
            ],
        ),
    ];

    for (bin, expected_diagnostics) in cases {
        let output = cargo_check(&manifest, &target_dir, Some(bin));
        assert!(!output.status.success(), "{bin} unexpectedly compiled");
        let stderr = String::from_utf8_lossy(&output.stderr);
        for expected in *expected_diagnostics {
            assert!(
                stderr.contains(expected),
                "{bin} omitted diagnostic `{expected}`:\n{stderr}"
            );
        }
        assert!(
            !stderr.contains("could not resolve the fe2o3-host crate"),
            "{bin} resolved host support before rejecting invalid syntax:\n{stderr}"
        );
    }
}
