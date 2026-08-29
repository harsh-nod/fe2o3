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
fn dynamic_matrix_kernel_reaches_gfx942_llvm() {
    assert_workgroup_pipeline_reaches_gfx942_llvm(
        "general-matrix",
        "examples/tiled_gemm_general_v1",
        "fe2o3_tiled_gemm_general_v1",
        "tiled_gemm_general_v1",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn dynamic_attention_kernel_reaches_gfx942_llvm() {
    assert_workgroup_pipeline_reaches_gfx942_llvm(
        "general-attention",
        "examples/flash_attention_general_v1",
        "fe2o3_flash_attention_general_v1",
        "flash_attention_general_v1",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn scalar_gemm_kernel_reaches_gfx942_llvm() {
    let scratch = ScratchDirectory::new("scalar-gemm");
    let example = workspace().join("examples/scalar_gemm_v1");
    let llvm_path = scratch.path.join("kernel.ll");
    let binding_path = scratch.path.join("crate-binding-v1");
    let output = Command::new(env!("CARGO"))
        .current_dir(&example)
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_scalar_gemm_v1")
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
        .expect("run scalar GEMM production gfx942 extraction");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        output.status.success(),
        "scalar GEMM did not complete production extraction:\n{stderr}"
    );
    assert!(
        stderr.contains("Rust -> semantic MIR -> ranked PLIRON -> Kernel IR")
            && stderr.contains("composed formal/ranked memory -> gfx942:xnack- LLVM")
            && stderr
                .contains("1 semantic u32 induction certificate(s) for 3 checked addition(s)",)
            && stderr.contains("artifact/launch authority false"),
        "scalar GEMM extraction omitted its successful lowering receipt:\n{stderr}"
    );
    for forbidden in ["error[FE2O3-RACE", "lowering stopped", "panic"] {
        assert!(
            !stderr.contains(forbidden),
            "scalar GEMM extraction emitted forbidden diagnostic {forbidden:?}:\n{stderr}"
        );
    }

    let llvm = std::fs::read_to_string(&llvm_path).expect("production extraction emitted LLVM");
    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "@scalar_gemm_v1",
        "llvm.amdgcn.workitem.id.x",
        "llvm.uadd.with.overflow.i32",
        "call { i32, i1 } @llvm.uadd.with.overflow.i32",
        "fmul float",
        "fadd float",
    ] {
        assert!(
            llvm.contains(required),
            "scalar GEMM production LLVM omitted {required:?}:\n{llvm}"
        );
    }
    for forbidden in ["llvm.fma", "llvm.fmuladd", "llvm.amdgcn.mfma"] {
        assert!(
            !llvm.contains(forbidden),
            "scalar GEMM production LLVM contains forbidden {forbidden:?}:\n{llvm}"
        );
    }
    let binding = std::fs::read_to_string(&binding_path).expect("crate binding handoff");
    assert_eq!(binding.trim().len(), 64);
    assert!(binding.trim().bytes().all(|byte| byte.is_ascii_hexdigit()));
}

fn assert_workgroup_pipeline_reaches_gfx942_llvm(
    label: &str,
    example_path: &str,
    crate_name: &str,
    kernel_symbol: &str,
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
        output.status.success(),
        "{crate_name} did not complete production extraction:\n{stderr}"
    );
    assert!(
        stderr.contains("Rust -> semantic MIR -> ranked PLIRON -> Kernel IR")
            && stderr.contains("composed formal/ranked memory -> gfx942:xnack- LLVM")
            && stderr.contains("artifact/launch authority false"),
        "production extraction omitted its successful lowering receipt:\n{stderr}"
    );
    for forbidden in ["error[FE2O3-RACE", "lowering stopped", "panic"] {
        assert!(
            !stderr.contains(forbidden),
            "production extraction emitted forbidden diagnostic {forbidden:?}:\n{stderr}"
        );
    }

    let llvm = std::fs::read_to_string(&llvm_path).expect("production extraction emitted LLVM");
    assert!(
        llvm.contains(&format!("@{kernel_symbol}"))
            && llvm.contains("llvm.amdgcn.mfma")
            && llvm.contains("addrspace(3)"),
        "production LLVM omitted the kernel, MFMA, or workgroup storage:\n{llvm}"
    );
    let binding = std::fs::read_to_string(&binding_path).expect("crate binding handoff");
    assert_eq!(binding.trim().len(), 64);
    assert!(binding.trim().bytes().all(|byte| byte.is_ascii_hexdigit()));
}
