use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

fn backend_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn backend_build(workspace: &Path, target: &str) -> Output {
    Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "-p",
            "fe2o3-half-math-compiler-fixture",
        ])
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-v1")
        .output()
        .expect("run half/math compiler fixture")
}

#[test]
fn rustc_authenticates_and_lowers_exact_half_math_source_forms() {
    let _lock = backend_test_lock();
    let output = backend_build(&workspace(), "gfx942");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "the fixture must stop at kernel-ir-v1's deliberately narrow kernel admission"
    );
    assert!(
        stderr.contains("selected kernel-ir-v1: verified 1 kernel(s), 8 function(s)"),
        "the exact source forms did not reach verified Kernel IR:\n{stderr}"
    );
    assert!(
        stderr.contains("does not support kernel export \"half_math_kernel\""),
        "the fixture failed before the expected post-translation admission boundary:\n{stderr}"
    );
    assert!(!stderr.contains("has no classified trusted device identity"));
}

#[test]
fn rustc_half_math_source_fails_closed_on_another_target() {
    let _lock = backend_test_lock();
    let output = backend_build(&workspace(), "gfx1100");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("requires the exact gfx942 floating-point profile"),
        "wrong-target source missed the target gate:\n{stderr}"
    );
    assert!(!stderr.contains("selected kernel-ir-v1: verified"));
}
