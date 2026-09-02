use std::path::Path;
use std::process::Command;

const FORBIDDEN_GPU_LIBRARIES: [&str; 6] = ["hsa", "hip", "kfd", "rocm", "amdgpu", "drm_amdgpu"];
const FORBIDDEN_LEGACY_RUNTIME_LIBRARIES: [&str; 4] = ["hsa", "hip", "rocm", "amdhip"];

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
fn direct_kfd_example_has_no_legacy_runtime_dynamic_dependency() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "build",
            "--locked",
            "--offline",
            "--package",
            "fe2o3-vecadd",
        ])
        .env(
            "FE2O3_CRATE_BINDING_ID_V1",
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .env_remove("FE2O3_HIP_SYS_DISABLE")
        .env_remove("FE2O3_HSA_RUNTIME_DISABLE")
        .output()
        .expect("build direct-KFD example");
    assert!(
        output.status.success(),
        "direct-KFD example build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let binary = target.join("debug/fe2o3-vecadd");
    let output = Command::new("readelf")
        .args(["--dynamic", binary.to_str().expect("UTF-8 target path")])
        .output()
        .expect("inspect direct-KFD example");
    assert!(
        output.status.success(),
        "readelf failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dynamic = String::from_utf8(output.stdout).expect("UTF-8 readelf output");
    let lower = dynamic.to_ascii_lowercase();
    for forbidden in FORBIDDEN_LEGACY_RUNTIME_LIBRARIES {
        assert!(
            !lower.contains(forbidden),
            "{} unexpectedly depends on a legacy runtime containing {forbidden}:\n{dynamic}",
            binary.display()
        );
    }
}

#[test]
fn production_dependency_graph_excludes_optional_runtime_and_oracle_crates() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dependency_tree = |package: &str, features: Option<&str>| {
        let mut command = Command::new(env!("CARGO"));
        command.current_dir(&workspace).args([
            "tree",
            "--package",
            package,
            "--edges",
            "normal",
            "--locked",
            "--offline",
            "--prefix",
            "none",
        ]);
        if let Some(features) = features {
            command.args(["--no-default-features", "--features", features]);
        }
        let output = command.output().expect("run cargo tree");
        assert!(
            output.status.success(),
            "cargo tree failed for {package}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("UTF-8 cargo tree output")
    };

    let tree = dependency_tree("cargo-fe2o3", None);
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

    let host_tree = dependency_tree("fe2o3-host", None);
    for package in [
        "fe2o3-core ",
        "fe2o3-hip-sys ",
        "fe2o3-hsa-runtime ",
        "fe2o3-worker-v2-bundle ",
    ] {
        assert!(
            !host_tree.lines().any(|line| line.starts_with(package)),
            "default fe2o3-host normal closure unexpectedly contains {package}:\n{host_tree}"
        );
    }
    for package in ["fe2o3-kfd ", "fe2o3-runtime ", "fe2o3-runtime-protocol "] {
        assert!(
            host_tree.lines().any(|line| line.starts_with(package)),
            "default fe2o3-host normal closure lacks {package}:\n{host_tree}"
        );
    }

    let vecadd_tree = dependency_tree("fe2o3-vecadd", None);
    for package in ["fe2o3-core ", "fe2o3-hip-sys ", "fe2o3-hsa-runtime "] {
        assert!(
            !vecadd_tree.lines().any(|line| line.starts_with(package)),
            "default fe2o3-vecadd normal closure unexpectedly contains {package}:\n{vecadd_tree}"
        );
    }
    for package in ["fe2o3-host ", "fe2o3-kfd ", "fe2o3-runtime "] {
        assert!(
            vecadd_tree.lines().any(|line| line.starts_with(package)),
            "default fe2o3-vecadd normal closure lacks {package}:\n{vecadd_tree}"
        );
    }

    let core_tree = dependency_tree("fe2o3-core", None);
    assert!(
        !core_tree
            .lines()
            .any(|line| line.starts_with("fe2o3-hip-sys ")),
        "default fe2o3-core normal closure unexpectedly contains HIP:\n{core_tree}"
    );

    let hsa_tree = dependency_tree("fe2o3-hsa-runtime", None);
    for package in ["fe2o3-core ", "fe2o3-host ", "fe2o3-hip-sys "] {
        assert!(
            !hsa_tree.lines().any(|line| line.starts_with(package)),
            "default fe2o3-hsa-runtime normal closure unexpectedly contains {package}:\n{hsa_tree}"
        );
    }

    let legacy_core_tree = dependency_tree("fe2o3-core", Some("qualification-legacy-hip-runtime"));
    assert!(
        legacy_core_tree
            .lines()
            .any(|line| line.starts_with("fe2o3-hip-sys ")),
        "explicit legacy fe2o3-core qualification lacks HIP bindings:\n{legacy_core_tree}"
    );

    let legacy_hsa_tree = dependency_tree(
        "fe2o3-hsa-runtime",
        Some("qualification-legacy-hsa-runtime"),
    );
    for package in ["fe2o3-core ", "fe2o3-host ", "fe2o3-hip-sys "] {
        assert!(
            legacy_hsa_tree
                .lines()
                .any(|line| line.starts_with(package)),
            "explicit legacy HSA qualification lacks {package}:\n{legacy_hsa_tree}"
        );
    }
}
