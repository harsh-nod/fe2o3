use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-{label}-{}-{nonce}",
            std::process::id(),
        ));
        std::fs::create_dir(&path).expect("create production matrix scratch directory");
        Self { path }
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn dynamic_matrix_kernel_fails_closed_before_lowering_without_race_proof() {
    assert_workgroup_pipeline_reaches_race_boundary(
        "general-matrix",
        "examples/tiled_gemm_general_v1",
        "fe2o3_tiled_gemm_general_v1",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn dynamic_attention_kernel_fails_closed_after_pipeline_verification() {
    assert_workgroup_pipeline_reaches_race_boundary(
        "general-attention",
        "examples/flash_attention_general_v1",
        "fe2o3_flash_attention_general_v1",
    );
}

fn assert_workgroup_pipeline_reaches_race_boundary(
    label: &str,
    example_path: &str,
    crate_name: &str,
) {
    let scratch = ScratchDirectory::new(label);
    let example = workspace().join(example_path);
    let llvm_path = scratch.path.join("kernel.ll");
    let binding_path = scratch.path.join("crate-binding-v1");
    let output = Command::new(env!("CARGO"))
        .current_dir(&example)
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env("FE2O3_EXTRACT_CRATE_V1", crate_name)
        .env("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1", &binding_path)
        .env("FE2O3_EXTRACT_GFX942_LLVM_PATH_V1", &llvm_path)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--release",
            "--locked",
            "-Zbuild-std=core",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(scratch.path.join("cargo"))
        .arg("--lib")
        .output()
        .expect("run production gfx942 extraction");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        !output.status.success(),
        "{crate_name} unexpectedly bypassed the production race proof boundary"
    );
    assert!(
        stderr.contains(
            "error[FE2O3-RACE-002]: cannot prove race freedom for dynamic launch dimension 0"
        ) && stderr.contains(
            "help: retain a bounded launch contract or supply a symbolic disjointness proof"
        ) && stderr.contains("ranked PLIRON before rejected lowering:")
            && stderr.contains("lowering stopped before target IR or artifact emission")
            && stderr.contains("kernel.pipeline_create")
            && stderr.contains("kernel.pipeline_event")
            && stderr.contains("Workgroup"),
        "production extraction omitted the exact fail-closed race diagnostic:\n{stderr}"
    );
    for forbidden in [
        "kernel-ir-v1",
        "kernel-ir-worker-v2",
        "collected-general-gemm-v1",
        "reached verified symbolic semantic template",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "production extraction entered forbidden route {forbidden:?}:\n{stderr}"
        );
    }

    assert!(
        !llvm_path.exists(),
        "rejected production analysis emitted target LLVM"
    );
}
