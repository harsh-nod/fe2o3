//! Protected MI300X evidence for the exact LDS GEMM Slice1 Worker V2 route.
//!
//! `fe2o3-hsa-runtime` intentionally does not depend directly on the compiler
//! import and finalizer crates. The ignored test therefore compiles the
//! committed runner under `tests/support` as an isolated Cargo package with
//! the exact workspace crates as direct path dependencies. The runner, rather
//! than this harness, owns every sealed value in the protected route.

#![cfg(target_os = "linux")]

#[cfg(feature = "hardware-test-hooks")]
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(feature = "hardware-test-hooks")]
const OPT_IN: &str = "FE2O3_RUN_GFX942_TILED_GEMM_LDS_SLICE1_WORKER_V2_HARDWARE";
#[cfg(feature = "hardware-test-hooks")]
const SUCCESS_MARKER: &str = "FE2O3_PROTECTED_SLICE1_WORKER_V2_OK";

#[cfg(feature = "hardware-test-hooks")]
struct TestDirectory(PathBuf);

#[cfg(feature = "hardware-test-hooks")]
impl TestDirectory {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = env::temp_dir().join(format!(
            "fe2o3-protected-slice1-runner-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

#[cfg(feature = "hardware-test-hooks")]
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn toml_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

#[cfg(feature = "hardware-test-hooks")]
fn runner_manifest(workspace: &Path) -> String {
    let dependency = |name: &str| {
        format!(
            "{name} = {{ path = \"{}\" }}\n",
            toml_path(&workspace.join("crates").join(name))
        )
    };
    let mut manifest = String::from(
        "[package]\n\
         name = \"fe2o3-protected-slice1-hardware-runner\"\n\
         version = \"0.1.0\"\n\
         edition = \"2024\"\n\
         rust-version = \"1.94\"\n\n\
         [workspace]\n\n\
         [dependencies]\n",
    );
    for name in [
        "dialect-amdgcn",
        "fe2o3-artifact-transaction",
        "fe2o3-artifacts",
        "fe2o3-compiler-ffi",
        "fe2o3-core",
        "fe2o3-host",
        "fe2o3-hsa-runtime",
        "fe2o3-hsaco-finalize",
        "fe2o3-kernel-descriptor",
        "fe2o3-kernel-ir",
    ] {
        manifest.push_str(&dependency(name));
    }
    manifest.push_str("sha2 = { version = \"0.11.0\", default-features = false }\n");
    manifest
}

#[cfg(feature = "hardware-test-hooks")]
fn require_environment() -> Result<(), Box<dyn std::error::Error>> {
    if env::var(OPT_IN).as_deref() != Ok("1") {
        return Err(format!("set {OPT_IN}=1 to opt in").into());
    }
    for variable in [
        "FE2O3_LDS_GEMM_V1_WORKER",
        "FE2O3_LDS_GEMM_V1_WORKER_BUILD_ID",
        "FE2O3_LDS_GEMM_V1_LLVM_BUILD_ID",
    ] {
        if env::var_os(variable).is_none() {
            return Err(format!("set {variable} for the measured #97 Worker V2").into());
        }
    }
    Ok(())
}

/// Executes the complete protected Slice1 route on gfx942:xnack-.
///
/// The outer package lacks direct test dependencies on the sealed compiler and
/// finalizer APIs. Its isolated runner has those dependencies without changing
/// this crate's production or test manifest. The runner uses #97's measured
/// direct upstream-LLVM API worker; it does not invoke COMGR, `llc`, or
/// `ld.lld` as a shell linker.
///
/// ```text
/// FE2O3_RUN_GFX942_TILED_GEMM_LDS_SLICE1_WORKER_V2_HARDWARE=1 \
/// HSA_XNACK=0 HIP_VISIBLE_DEVICES=0 ROCR_VISIBLE_DEVICES=0 \
/// FE2O3_LDS_GEMM_V1_WORKER=/absolute/fe2o3-llvm-link-worker \
/// FE2O3_LDS_GEMM_V1_WORKER_BUILD_ID=<measured-worker-build-id> \
/// FE2O3_LDS_GEMM_V1_LLVM_BUILD_ID=<upstream-llvm-build-id> \
/// cargo test --locked -p fe2o3-hsa-runtime --features hardware-test-hooks \
///   --test tiled_gemm_lds_slice1_worker_v2_hardware \
///   gfx942_tiled_gemm_lds_slice1_worker_v2_protected_hardware \
///   -- --ignored --exact --nocapture
/// ```
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "requires the measured #97 upstream-LLVM API worker and gfx942:xnack-"]
fn gfx942_tiled_gemm_lds_slice1_worker_v2_protected_hardware()
-> Result<(), Box<dyn std::error::Error>> {
    require_environment()?;
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("HSA runtime crate is not under the workspace crates directory")?
        .to_path_buf();
    let runner_source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/support/tiled_gemm_lds_slice1_worker_v2_runner.rs");
    if !runner_source.is_file() {
        return Err(format!("missing protected runner at {}", runner_source.display()).into());
    }

    let project = TestDirectory::new()?;
    fs::create_dir(project.0.join("src"))?;
    fs::write(project.0.join("Cargo.toml"), runner_manifest(&workspace))?;
    fs::copy(&runner_source, project.0.join("src/main.rs"))?;

    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let target_dir = env::var_os("FE2O3_PROTECTED_SLICE1_RUNNER_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target/protected-slice1-worker-v2-runner"));
    let output = Command::new(cargo)
        .args([
            "run",
            "--offline",
            "--quiet",
            "--manifest-path",
            project.0.join("Cargo.toml").to_str().ok_or("UTF-8 path")?,
        ])
        .env("CARGO_TARGET_DIR", target_dir)
        .env("FE2O3_WORKSPACE_ROOT", &workspace)
        .env_remove("FE2O3_LLC")
        .env_remove("FE2O3_LLD")
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!(
            "protected Slice1 runner failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        )
        .into());
    }
    if !stdout.lines().any(|line| line.starts_with(SUCCESS_MARKER)) {
        return Err(format!(
            "protected Slice1 runner omitted success marker\nstdout:\n{}\nstderr:\n{}",
            stdout, stderr
        )
        .into());
    }
    print!("{stdout}");
    Ok(())
}
