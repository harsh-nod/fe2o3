use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest as _, Sha256};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

struct ScratchTarget {
    path: PathBuf,
}

impl ScratchTarget {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("fe2o3-production-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path).expect("create isolated Cargo target directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchTarget {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn cargo_fe2o3_executable(workspace: &Path, cargo_build_target: &Path) -> PathBuf {
    let mut build = Command::new(env!("CARGO"));
    build
        .current_dir(workspace)
        .args(["build", "--locked", "--target-dir"])
        .arg(cargo_build_target)
        .args(["-p", "cargo-fe2o3", "--bin", "cargo-fe2o3"]);
    remove_rustc_overrides(&mut build);
    let output = build.output().expect("build cargo-fe2o3 executable");
    assert!(
        output.status.success(),
        "cargo-fe2o3 build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let executable = cargo_build_target
        .join("debug")
        .join(format!("cargo-fe2o3{}", std::env::consts::EXE_SUFFIX));
    assert!(
        executable.is_file(),
        "cargo-fe2o3 executable is missing at {}",
        executable.display()
    );
    executable
}

fn production_build_config(workspace: &Path, isolated_target: &Path) -> PathBuf {
    let worker = std::env::current_exe().expect("current production pipeline test executable");
    let bytes = std::fs::read(&worker).expect("read inert production worker executable");
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let config = isolated_target.join("production-build-config.json");
    let worker = worker
        .to_str()
        .expect("UTF-8 production worker executable path");
    let workspace = workspace.to_str().expect("UTF-8 workspace path");
    let json = format!(
        "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-production-build-config-v1\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"5\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"units\":[{{\"crate_name\":\"fe2o3_fill\",\"source\":\"examples/fill/src/lib.rs\",\"working_directory\":{workspace:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":\"test-only-unreached-llvm\",\"path\":{worker:?},\"sha256\":\"{digest}\",\"worker_build_identity\":\"test-only-unreached-worker\"}}}}",
        bytes.len()
    );
    std::fs::write(&config, json).expect("write canonical inert production build config");
    config
}

fn remove_rustc_overrides(process: &mut Command) {
    for variable in [
        "RUSTC",
        "CARGO_BUILD_RUSTC",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        process.env_remove(variable);
    }
}

fn cargo_fe2o3(
    workspace: &Path,
    executable: &Path,
    isolated_target: &Path,
    build_config: &Path,
    command: &str,
    package: &str,
) -> Output {
    let mut process = Command::new(executable);
    process
        .current_dir(workspace)
        .env("FE2O3_TARGET", "gfx942")
        .env("FE2O3_PRODUCTION_BUILD_CONFIG_V1", build_config)
        .env("CARGO_TARGET_DIR", isolated_target)
        .env_remove("LD_LIBRARY_PATH")
        .args([command, "-p", package]);
    remove_rustc_overrides(&mut process);
    process.output().expect("run cargo-fe2o3")
}

#[test]
#[ignore = "requires the configured gfx942 cargo-fe2o3 compiler toolchain"]
fn attributed_kernel_enters_one_transaction_and_fails_without_fallback() {
    let workspace = workspace();
    let configured_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let cargo_build_target = if configured_target.is_absolute() {
        configured_target
    } else {
        workspace.join(configured_target)
    };
    let isolated_target = ScratchTarget::new();
    let executable = cargo_fe2o3_executable(&workspace, &cargo_build_target);
    let build_config = production_build_config(&workspace, isolated_target.path());

    let output = cargo_fe2o3(
        &workspace,
        &executable,
        isolated_target.path(),
        &build_config,
        "build",
        "fe2o3-fill",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "unprotected production transaction unexpectedly succeeded"
    );
    assert!(
        stderr.contains(
            "cargo fe2o3 authority release requires a protected pre-exec launcher/image contract; this build has no admitted release launcher"
        ),
        "missing fail-closed production diagnostic:\n{stderr}"
    );
    for forbidden in [
        "kernel-ir-v1",
        "kernel-ir-worker-v2",
        "collected-executable-scalar-control-flow-v2",
        "collected-flash-attention-v1",
        "collected-general-gemm-v1",
        "collected-moe-top2-v1",
        "collected-row-softmax-v1",
        "collected-scalar-gemm-v1",
        "collected-tiled-gemm-v1",
        "collected-wave64-collectives-v1",
        "collected-lds-reduction-v1",
        "collected-scoped-atomic-v1",
        "semantic importer authenticated rustc target",
        "selected canonical Kernel IR module",
        "emitted fill",
        "published inert production worker",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "production transaction entered another compiler route {forbidden:?}:\n{stderr}"
        );
    }
    let artifact_directory = isolated_target.path().join("fe2o3");
    assert!(
        !artifact_directory.exists(),
        "failed production transaction retained managed artifact state at {}",
        artifact_directory.display()
    );
}
