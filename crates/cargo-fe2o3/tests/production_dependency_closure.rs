use std::path::Path;
use std::process::Command;

const FORBIDDEN_GPU_LIBRARIES: [&str; 6] = ["hsa", "hip", "kfd", "rocm", "amdgpu", "drm_amdgpu"];

#[test]
fn production_binary_has_no_simulation_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["simulate", "--help"])
        .env_clear()
        .output()
        .expect("probe removed simulation command");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 command error");
    assert!(stderr.contains("unknown cargo-fe2o3 command"), "{stderr}");
    assert!(!stderr.contains("compile source to exact KIR"), "{stderr}");
}

#[test]
fn production_binary_has_no_gpu_runtime_dynamic_dependency() {
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
    for forbidden in FORBIDDEN_GPU_LIBRARIES {
        assert!(
            !lower.contains(forbidden),
            "{binary} unexpectedly depends on a GPU runtime containing {forbidden}:\n{dynamic}"
        );
    }
}

#[test]
fn production_dependency_graph_excludes_optional_runtime_and_oracle_crates() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dependency_tree = |package: &str| {
        let output = Command::new(env!("CARGO"))
            .current_dir(&workspace)
            .args([
                "tree",
                "--package",
                package,
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
            "cargo tree failed for {package}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("UTF-8 cargo tree output")
    };

    let tree = dependency_tree("cargo-fe2o3");
    for package in [
        "fe2o3-core ",
        "fe2o3-host ",
        "fe2o3-hsa-runtime ",
        "fe2o3-hip-sys ",
        "fe2o3-kir-sim-cli ",
        "fe2o3-worker-v2-bundle ",
    ] {
        assert!(
            !tree.lines().any(|line| line.starts_with(package)),
            "default cargo-fe2o3 normal closure unexpectedly contains {package}:\n{tree}"
        );
    }
    assert!(
        tree.lines()
            .any(|line| line.starts_with("fe2o3-runtime-protocol ")),
        "default cargo-fe2o3 normal closure lacks the production runtime protocol:\n{tree}"
    );

    let host_tree = dependency_tree("fe2o3-host");
    assert!(
        !host_tree
            .lines()
            .any(|line| line.starts_with("fe2o3-worker-v2-bundle ")),
        "default fe2o3-host normal closure unexpectedly contains Worker V2:\n{host_tree}"
    );
    assert!(
        host_tree
            .lines()
            .any(|line| line.starts_with("fe2o3-runtime-protocol ")),
        "default fe2o3-host normal closure lacks the production runtime protocol:\n{host_tree}"
    );
}
