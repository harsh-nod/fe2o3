use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const PIPELINE_ENV: &str = "FE2O3_CODEGEN_PIPELINE";

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
        let path = std::env::temp_dir().join(format!(
            "fe2o3-production-v1-{}-{nonce}",
            std::process::id()
        ));
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

fn cargo_fe2o3(
    workspace: &Path,
    cargo_build_target: &Path,
    isolated_target: &Path,
    command: &str,
    package: &str,
) -> Output {
    let mut process = Command::new(env!("CARGO"));
    process
        .current_dir(workspace)
        .env(PIPELINE_ENV, "production-v1")
        .env("CARGO_TARGET_DIR", isolated_target)
        .args(["run", "--locked", "--target-dir"])
        .arg(cargo_build_target)
        .args(["-p", "cargo-fe2o3", "--", command, "-p", package]);
    for variable in [
        "RUSTC",
        "CARGO_BUILD_RUSTC",
        "RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        process.env_remove(variable);
    }
    process.output().expect("run cargo-fe2o3")
}

#[test]
#[ignore = "requires the configured gfx942 cargo-fe2o3 compiler toolchain"]
fn attributed_kernel_enters_one_transaction_and_fails_without_fallback() {
    let workspace = workspace();
    let cargo_build_target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("target"));
    let isolated_target = ScratchTarget::new();

    let output = cargo_fe2o3(
        &workspace,
        &cargo_build_target,
        isolated_target.path(),
        "build",
        "fe2o3-fill",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "unimplemented production transaction unexpectedly succeeded"
    );
    assert!(
        stderr.contains("generic semantic-MIR import transition is not implemented")
            && stderr.contains("1 registered kernel root(s)")
            && stderr.contains("transaction was consumed without fallback or artifact emission"),
        "missing fail-closed production diagnostic:\n{stderr}"
    );
    for forbidden in [
        "legacy-v1",
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
        "emitted fill",
        "published inert Worker V2",
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
