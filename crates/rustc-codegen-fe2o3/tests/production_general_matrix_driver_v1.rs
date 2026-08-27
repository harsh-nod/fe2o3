use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-general-matrix-{}-{nonce}",
            std::process::id()
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
fn safe_dynamic_matrix_kernel_uses_the_single_production_pipeline() {
    let scratch = ScratchDirectory::new();
    let example = workspace().join("examples/tiled_gemm_general_v1");
    let llvm_path = scratch.path.join("kernel.ll");
    let binding_path = scratch.path.join("crate-binding-v1");
    let output = Command::new(env!("CARGO"))
        .current_dir(&example)
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_tiled_gemm_general_v1",
        )
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
        output.status.success(),
        "safe dynamic matrix kernel failed production extraction:\n{stderr}"
    );
    assert!(
        stderr.contains(
            "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> Kernel IR -> composed formal/ranked memory -> gfx942:xnack- LLVM;"
        )
            && stderr.contains("semantic function(s)")
            && stderr.contains("correspondence block(s)")
            && stderr.contains("ranked dynamic-index discharge(s)")
            && stderr.contains("artifact/launch authority false"),
        "production extraction omitted mandatory pipeline evidence:\n{stderr}"
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

    let llvm = std::fs::read_to_string(&llvm_path).expect("read extracted gfx942 LLVM");
    assert_eq!(
        llvm.matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
            .count(),
        1,
        "production LLVM must contain exactly one MFMA operation in the dynamic K-loop body",
    );
    assert!(
        llvm.contains("target triple = \"amdgcn-amd-amdhsa\"")
            && llvm.matches("define amdgpu_kernel").count() == 1
            && llvm.contains("!reqd_work_group_size"),
        "production LLVM omitted the exact AMDGPU kernel ABI",
    );
    let binding = std::fs::read_to_string(&binding_path).expect("read crate binding handoff");
    assert!(
        binding.trim().len() == 64 && binding.trim().bytes().all(|byte| byte.is_ascii_hexdigit()),
        "extractor emitted a malformed crate binding handoff",
    );
}
