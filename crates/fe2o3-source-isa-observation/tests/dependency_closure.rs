use std::process::Command;

use serde_json::Value;

#[test]
fn authority_free_dependency_closure_is_exact() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version=1",
            "--no-deps",
            "--locked",
            "--manifest-path",
            manifest,
        ])
        .output()
        .expect("run cargo metadata for source/ISA observation crate");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value = serde_json::from_slice(&output.stdout).expect("valid metadata JSON");
    let package = metadata["packages"]
        .as_array()
        .expect("metadata packages")
        .iter()
        .find(|package| package["name"] == "fe2o3-source-isa-observation")
        .expect("source/ISA observation package");
    let mut dependencies = package["dependencies"]
        .as_array()
        .expect("package dependencies")
        .iter()
        .map(|dependency| {
            (
                dependency["name"].as_str().expect("dependency name"),
                dependency["kind"].as_str().unwrap_or("normal"),
            )
        })
        .collect::<Vec<_>>();
    dependencies.sort_unstable();
    assert_eq!(
        dependencies,
        [
            ("fe2o3-kernel-ir", "normal"),
            ("serde", "normal"),
            ("serde_json", "normal"),
            ("sha2", "normal"),
        ]
    );
    for forbidden in [
        "fe2o3-artifact-transaction",
        "fe2o3-build-authority",
        "fe2o3-hsaco-finalize",
        "fe2o3-host",
        "fe2o3-hsa-runtime",
        "fe2o3-kfd",
        "hip-sys",
    ] {
        assert!(
            dependencies.iter().all(|(name, _)| *name != forbidden),
            "authority-free observer depends on {forbidden}"
        );
    }
}
