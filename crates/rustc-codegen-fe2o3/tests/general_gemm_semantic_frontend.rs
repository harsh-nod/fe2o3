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

fn cargo_target_directory(workspace: &Path) -> PathBuf {
    let Some(configured) = std::env::var_os("CARGO_TARGET_DIR") else {
        return workspace.join("target");
    };
    let configured = PathBuf::from(configured);
    if configured.is_absolute() {
        configured
    } else {
        workspace.join(configured)
    }
}

fn fixture(workspace: &Path) -> PathBuf {
    workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/general-gemm-semantic-frontend")
}

fn managed_build(
    workspace: &Path,
    manifest: &Path,
    cargo_args: &[&str],
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
        .arg(manifest)
        .args(cargo_args)
        .env(
            "FE2O3_BACKEND",
            cargo_target_directory(workspace).join("debug/librustc_codegen_fe2o3.so"),
        )
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-v1")
        .env("FE2O3_HSACO_DIR", artifacts)
        .output()
        .expect("run managed general GEMM frontend build")
}

fn contains_regular_file(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if metadata.is_file() {
        return true;
    }
    if !metadata.is_dir() {
        return false;
    }
    std::fs::read_dir(path)
        .expect("read artifact directory")
        .any(|entry| contains_regular_file(&entry.expect("read artifact entry").path()))
}

fn assert_failed_without_artifact(output: &Output, artifacts: &Path) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "general GEMM frontend unexpectedly published an artifact:\n{stderr}"
    );
    assert!(
        !contains_regular_file(artifacts),
        "general GEMM frontend failure left an artifact in {}:\n{stderr}",
        artifacts.display()
    );
    stderr
}

#[test]
fn safe_general_gemm_mir_reaches_kir_and_two_semantic_failures_are_diagnostic() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let fixture = fixture(&workspace);
    let built = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build codegen backend");
    assert!(
        built.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );

    for source in [
        fixture.join("src/bin/missing_publish.rs"),
        fixture.join("src/bin/duplicate_store.rs"),
        fixture.join("src/bin/conditional_publish.rs"),
        fixture.join("src/bin/reversed_cycle.rs"),
        fixture.join("src/bin/store_loop.rs"),
    ] {
        let source = std::fs::read_to_string(&source).expect("read safe semantic fixture");
        assert!(source.contains("#![forbid(unsafe_code)]"));
        assert!(!source.contains("unsafe {"));
    }

    let impostor_fixture =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/general-gemm-provider-impostor");
    let impostor_artifacts = cargo_target_directory(&workspace).join(format!(
        "rustc-codegen-fe2o3-test-output/general-gemm-impostor-{}",
        std::process::id()
    ));
    let impostor = managed_build(
        &workspace,
        &impostor_fixture.join("Cargo.toml"),
        &["-p", "general-gemm-provider-impostor-consumer"],
        &impostor_artifacts,
    );
    let impostor_stderr = assert_failed_without_artifact(&impostor, &impostor_artifacts);
    assert!(
        impostor_stderr.contains(
            "trusted-provider rejection: diagnostic item `fe2o3_device_general_tiled_gemm_proof_acquire_v1`"
        ) && impostor_stderr.contains(
            "not bound to the reviewed `fe2o3_gemm_device_v1` compilation unit"
        ) && impostor_stderr
            .contains("outside the reviewed fe2o3-device source root"),
        "same-name external general GEMM provider crossed the reviewed source boundary:\n{impostor_stderr}"
    );

    let output_root = cargo_target_directory(&workspace).join(format!(
        "rustc-codegen-fe2o3-test-output/general-gemm-semantic-{}",
        std::process::id()
    ));
    let positive_artifacts = output_root.join("positive");
    let positive = managed_build(
        &workspace,
        &workspace.join("examples/tiled_gemm_general_v1/Cargo.toml"),
        &["--release", "-p", "fe2o3-tiled-gemm-general-v1", "--lib"],
        &positive_artifacts,
    );
    let positive_stderr = assert_failed_without_artifact(&positive, &positive_artifacts);
    assert!(
        positive_stderr.contains(
            "authenticated Typestate general GEMM MIR reached verified semantic KIR witness"
        ) && positive_stderr
            .contains("runtime plan binding, frontend promotion, and lowering are not implemented"),
        "positive safe source missed the authenticated semantic KIR boundary:\n{positive_stderr}"
    );

    for (bin, code, property, stage) in [
        ("missing-publish", "0x46470103", "initialized", "gpu"),
        (
            "duplicate-store",
            "0x46470106",
            "output_region_injective",
            "tile",
        ),
    ] {
        let artifacts = output_root.join(bin);
        let rejected = managed_build(
            &workspace,
            &fixture.join("Cargo.toml"),
            &["--release", "--bin", bin],
            &artifacts,
        );
        let stderr = assert_failed_without_artifact(&rejected, &artifacts);
        assert!(
            stderr.contains(&format!(
                "authenticated general GEMM semantic KIR rejected: general GEMM {property} counterexample at {stage}"
            )) && stderr.contains(code),
            "safe semantic fixture `{bin}` missed exact {code} diagnostic:\n{stderr}"
        );
        assert!(
            !stderr.contains("reached verified semantic KIR witness"),
            "safe semantic fixture `{bin}` acquired a verified witness:\n{stderr}"
        );
    }

    for bin in ["conditional-publish", "reversed-cycle", "store-loop"] {
        let artifacts = output_root.join(bin);
        let rejected = managed_build(
            &workspace,
            &fixture.join("Cargo.toml"),
            &["--release", "--bin", bin],
            &artifacts,
        );
        let stderr = assert_failed_without_artifact(&rejected, &artifacts);
        assert!(
            stderr.contains(
                "general GEMM authenticated MIR import failed: proof-sensitive general GEMM"
            ),
            "safe hostile CFG fixture `{bin}` missed fail-closed MIR admission:\n{stderr}"
        );
        assert!(
            !stderr.contains("reached verified semantic KIR witness"),
            "safe hostile CFG fixture `{bin}` acquired a verified witness:\n{stderr}"
        );
    }

    let _ = std::fs::remove_dir_all(output_root);
}
