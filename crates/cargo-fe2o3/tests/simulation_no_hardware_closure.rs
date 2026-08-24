#![cfg(all(target_os = "linux", not(feature = "hardware-runtime")))]

use std::path::Path;
use std::process::Command;

const FORBIDDEN: [&str; 6] = ["hsa", "hip", "kfd", "rocm", "amdgpu", "drm_amdgpu"];

#[test]
fn simulation_help_requires_no_input_or_hardware_environment() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["simulate", "--help"])
        .env_clear()
        .output()
        .expect("run simulation help");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "usage: cargo fe2o3 simulate --request PATH [--output PATH] [-- CARGO_BUILD_ARGS...]\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn default_binary_has_no_gpu_runtime_dynamic_dependency() {
    let binary = env!("CARGO_BIN_EXE_cargo-fe2o3");
    let output = Command::new("readelf")
        .args(["--dynamic", binary])
        .output()
        .expect("run readelf");
    assert!(
        output.status.success(),
        "readelf failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dynamic = String::from_utf8(output.stdout).expect("UTF-8 readelf output");
    let lower = dynamic.to_ascii_lowercase();
    for forbidden in FORBIDDEN {
        assert!(
            !lower.contains(forbidden),
            "{binary} unexpectedly has GPU runtime dynamic dependency containing {forbidden}:\n{dynamic}"
        );
    }
}

#[test]
fn default_normal_dependency_graph_excludes_gpu_runtime_crates() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "tree",
            "--package",
            "cargo-fe2o3",
            "--edges",
            "normal",
            "--no-default-features",
            "--prefix",
            "none",
        ])
        .output()
        .expect("run cargo tree");
    assert!(
        output.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tree = String::from_utf8(output.stdout).expect("UTF-8 cargo tree output");
    for package in [
        "fe2o3-core ",
        "fe2o3-host ",
        "fe2o3-hsa-runtime ",
        "fe2o3-hip-sys ",
    ] {
        assert!(
            !tree.lines().any(|line| line.starts_with(package)),
            "default cargo-fe2o3 normal closure unexpectedly contains {package}:\n{tree}"
        );
    }
}
