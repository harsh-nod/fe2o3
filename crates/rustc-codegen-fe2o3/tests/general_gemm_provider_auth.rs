use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn contains_regular_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.is_file() {
        return true;
    }
    metadata.is_dir()
        && std::fs::read_dir(path)
            .expect("read artifact directory")
            .any(|entry| contains_regular_file(&entry.expect("read artifact entry").path()))
}

fn managed_build(
    workspace: &Path,
    target: &Path,
    fixture: &str,
    package: &str,
    artifacts: &Path,
) -> Output {
    let _ = std::fs::remove_dir_all(artifacts);
    Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "--locked",
            "--manifest-path",
        ])
        .arg(
            workspace
                .join("crates/rustc-codegen-fe2o3/tests/fixtures")
                .join(fixture)
                .join("Cargo.toml"),
        )
        .args(["--release", "-p", package])
        .env("CARGO_TARGET_DIR", target)
        .env(
            "FE2O3_BACKEND",
            target.join("debug/librustc_codegen_fe2o3.so"),
        )
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-v1")
        .env("FE2O3_HSACO_DIR", artifacts)
        .output()
        .expect("run managed provider-auth build")
}

fn failed_stderr(output: &Output, artifacts: &Path) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "build unexpectedly succeeded:\n{stderr}"
    );
    assert!(
        !contains_regular_file(artifacts),
        "failed build left an artifact in {}:\n{stderr}",
        artifacts.display()
    );
    stderr
}

#[test]
fn general_gemm_provider_auth_is_portable_and_semantic_not_manifest_provenance() {
    let workspace = workspace();
    let target = workspace.join(format!(
        "target/rustc-codegen-fe2o3-provider-auth-{}",
        std::process::id()
    ));
    let built = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .expect("build isolated codegen backend");
    assert!(
        built.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let partial_artifacts = target.join("partial-include-artifacts");
    let partial = managed_build(
        &workspace,
        &target,
        "general-gemm-provider-impostor",
        "general-gemm-provider-impostor-consumer",
        &partial_artifacts,
    );
    let partial_stderr = failed_stderr(&partial, &partial_artifacts);
    assert!(
        partial_stderr.contains(
            "trusted-provider rejection: diagnostic item `fe2o3_device_general_tiled_gemm_proof_acquire_v1`"
        )
            && partial_stderr.contains(
                "not bound to the reviewed `fe2o3_gemm_device_v1` compilation unit"
            )
            && partial_stderr.contains("outside the reviewed fe2o3-device source root"),
        "partial terminal include did not fail at provider-owned context source:\n{partial_stderr}"
    );

    let dependency_artifacts = target.join("dependency-impostor-artifacts");
    let dependency = managed_build(
        &workspace,
        &target,
        "general-gemm-provider-dependency-impostor",
        "general-gemm-provider-dependency-impostor-consumer",
        &dependency_artifacts,
    );
    let dependency_stderr = failed_stderr(&dependency, &dependency_artifacts);
    assert!(
        dependency_stderr.contains(
            "trusted-provider rejection: diagnostic item `fe2o3_device_general_tiled_gemm_proof_acquire_v1`"
        )
            && dependency_stderr.contains(
                "not bound to the reviewed `fe2o3_gemm_device_v1` compilation unit"
            )
            && dependency_stderr.contains("substituted its fe2o3_device DisjointSlice dependency"),
        "substituted dependency crossed the compiled FnSig boundary:\n{dependency_stderr}"
    );

    let equivalent_artifacts = target.join("equivalent-manifest-artifacts");
    let equivalent = managed_build(
        &workspace,
        &target,
        "general-gemm-provider-equivalent-manifest",
        "general-gemm-provider-equivalent-consumer",
        &equivalent_artifacts,
    );
    let equivalent_stderr = failed_stderr(&equivalent, &equivalent_artifacts);
    assert!(
        equivalent_stderr.contains("0x46470103")
            && equivalent_stderr.contains("general GEMM initialized counterexample")
            && !equivalent_stderr.contains("trusted-provider rejection"),
        "semantically identical alternate manifest did not cross provider auth:\n{equivalent_stderr}"
    );

    let _ = std::fs::remove_dir_all(target);
}
