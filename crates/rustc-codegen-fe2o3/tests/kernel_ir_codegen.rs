use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

const PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";

fn backend_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("backend test lock")
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn backend(workspace: &Path, command: &str, package: &str, pipeline: Option<&str>) -> Output {
    let mut process = Command::new(env!("CARGO"));
    process
        .current_dir(workspace)
        .env_remove(PIPELINE_ENV)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            command,
            "-p",
            package,
        ]);
    if let Some(pipeline) = pipeline {
        process.env(PIPELINE_ENV, pipeline);
    }
    process.output().expect("run cargo-fe2o3")
}

fn artifact_paths(workspace: &Path, kernel: &str) -> [PathBuf; 3] {
    let directory = workspace.join("target/fe2o3");
    ["ll", "o", "hsaco"].map(|extension| directory.join(format!("{kernel}.{extension}")))
}

fn preseed(paths: &[PathBuf]) {
    std::fs::create_dir_all(paths[0].parent().expect("artifact parent"))
        .expect("create artifact directory");
    for path in paths {
        std::fs::write(path, b"preseeded stale artifact")
            .unwrap_or_else(|error| panic!("preseed {}: {error}", path.display()));
    }
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain and a local AMD GPU"]
fn opt_in_fill_publishes_g1_and_executes_on_the_gpu() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let output = backend(&workspace, "run", "fe2o3-fill", Some("kernel-ir-v1"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "kernel-ir-v1 fill failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("selected kernel-ir-v1: verified 1 kernel(s), 3 function(s)"),
        "missing selected-pipeline diagnostic:\n{stderr}"
    );
    assert!(
        stderr.contains("emitted fill"),
        "fill was not transactionally published:\n{stderr}"
    );
    assert!(
        stdout.contains("fill passed for 1024 elements"),
        "fill did not execute successfully:\n{stdout}"
    );

    let llvm = std::fs::read_to_string(workspace.join("target/fe2o3/fill.ll"))
        .expect("published fill LLVM IR");
    assert!(llvm.contains("define amdgpu_kernel void @fill"));
    assert!(llvm.contains("mul i64 %v1.group, 256"));
    assert!(llvm.contains("!reqd_work_group_size !0"));
    assert!(!llvm.contains("%base = mul i32 %bid, 256"));
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain and a local AMD GPU"]
fn opt_in_vecadd_publishes_exact_g1_and_executes_on_the_gpu() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let output = backend(&workspace, "run", "fe2o3-vecadd", Some("kernel-ir-v1"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "kernel-ir-v1 vecadd failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("selected kernel-ir-v1: verified 1 kernel(s), 4 function(s)"),
        "missing selected-pipeline diagnostic:\n{stderr}"
    );
    assert!(
        stderr.contains("emitted vecadd"),
        "vecadd was not transactionally published:\n{stderr}"
    );
    assert!(
        stdout.contains("vecadd passed for 1024 elements"),
        "vecadd did not execute successfully:\n{stdout}"
    );

    let llvm = std::fs::read_to_string(workspace.join("target/fe2o3/vecadd.ll"))
        .expect("published vecadd LLVM IR");
    assert!(llvm.contains(
        "@vecadd(ptr addrspace(1) %arg0.data, i64 %arg0.len, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len)"
    ));
    assert_eq!(llvm.matches("load float").count(), 2);
    assert_eq!(llvm.matches("store float").count(), 1);
    assert_eq!(llvm.matches("fadd float").count(), 1);
    assert!(llvm.contains("!reqd_work_group_size !0"));
    assert!(!llvm.contains("fe2o3_device"));
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn selected_pipeline_rejects_invalid_or_unsupported_inputs_and_cleans_stale_artifacts() {
    let _lock = backend_test_lock();
    let workspace = workspace();

    let vecadd_artifacts = artifact_paths(&workspace, "vecadd");
    preseed(&vecadd_artifacts);
    let invalid = backend(&workspace, "build", "fe2o3-vecadd", Some("kernel-ir"));
    let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(!invalid.status.success(), "invalid selector was accepted");
    assert!(
        invalid_stderr.contains(
            "FE2O3_CODEGEN_PIPELINE must be unset or exactly `legacy-v1` or `kernel-ir-v1`"
        ),
        "missing strict selector diagnostic:\n{invalid_stderr}"
    );
    assert!(!invalid_stderr.contains("emitted vecadd"));
    for artifact in vecadd_artifacts {
        assert!(
            !artifact.exists(),
            "invalid selector left stale artifact {}",
            artifact.display()
        );
    }

    let copy_artifacts = artifact_paths(&workspace, "copy");
    preseed(&copy_artifacts);
    let unsupported = backend(&workspace, "build", "fe2o3-copy", Some("kernel-ir-v1"));
    let unsupported_stderr = String::from_utf8_lossy(&unsupported.stderr);
    assert!(
        !unsupported.status.success(),
        "unsupported selected kernel unexpectedly compiled"
    );
    assert!(
        unsupported_stderr.contains("does not support kernel export \"copy\""),
        "missing exact admission diagnostic:\n{unsupported_stderr}"
    );
    assert!(
        unsupported_stderr.contains("default legacy-v1 pipeline"),
        "diagnostic did not identify the available legacy path:\n{unsupported_stderr}"
    );
    assert!(!unsupported_stderr.contains("emitted copy"));
    for artifact in copy_artifacts {
        assert!(
            !artifact.exists(),
            "unsupported selected kernel left stale artifact {}",
            artifact.display()
        );
    }
}
