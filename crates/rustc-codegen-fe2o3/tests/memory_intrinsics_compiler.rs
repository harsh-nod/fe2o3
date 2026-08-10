use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

use fe2o3_artifacts::DigestAlgorithm;

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

struct WorkerV2MissingEnvelope(PathBuf);

impl WorkerV2MissingEnvelope {
    fn new(workspace: &Path) -> Self {
        let worker = std::env::current_exe().expect("current test executable");
        let bytes = std::fs::read(&worker).expect("read current test executable");
        let digest = DigestAlgorithm::Sha256.calculate(&bytes).bytes();
        let digest = digest
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let fixture_source =
            "crates/rustc-codegen-fe2o3/tests/fixtures/memory-v1-compiler/src/main.rs";
        let path = workspace.join(format!(
            "target/memory-v1-worker-v2-missing-envelope-{}.json",
            std::process::id()
        ));
        let worker = worker.to_str().expect("UTF-8 worker path");
        let source = fixture_source;
        let working_directory = workspace.to_str().expect("UTF-8 workspace path");
        let json = format!(
            "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-worker-v2-config-v2\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"6\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"units\":[{{\"crate_name\":\"fe2o3_memory_v1_compiler_fixture\",\"source\":{source:?},\"working_directory\":{working_directory:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":\"test-only-unreached-llvm\",\"path\":{worker:?},\"sha256\":\"{digest}\",\"worker_build_identity\":\"test-only-unreached-worker\"}}}}",
            bytes.len()
        );
        std::fs::write(&path, json).expect("write Worker V2 test config");
        Self(path)
    }
}

impl Drop for WorkerV2MissingEnvelope {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn backend_build(workspace: &Path, target: &str) -> Output {
    let config = WorkerV2MissingEnvelope::new(workspace);
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
            "fe2o3-memory-v1-compiler-fixture",
        ])
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config.0)
        .output()
        .expect("run memory-v1 compiler fixture")
}

#[test]
fn rustc_authenticates_memory_api_and_reaches_verified_kernel_ir() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let output = backend_build(&workspace, "gfx942:xnack-");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "the fixture must stop at the deliberately missing Worker V2 envelope"
    );
    assert!(
        stderr.contains(
            "selected kernel-ir-worker-v2: verified compiler-module candidate with 1 kernel(s)"
        ),
        "the source memory calls did not reach verified Kernel IR:\n{stderr}"
    );
    assert!(
        stderr.contains("requires a complete compiler FFI envelope"),
        "the fixture missed the expected post-translation Worker V2 boundary:\n{stderr}"
    );
    assert!(!stderr.contains("has no classified trusted device identity"));
    assert!(!stderr.contains("MIR is unavailable for a device-reachable item"));
}

#[test]
fn rustc_memory_api_fails_closed_on_a_non_gfx942_target() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let output = backend_build(&workspace, "gfx1100");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(
        stderr.contains("requires the gfx942 General V3 memory-v1 profile"),
        "wrong-target source missed the memory profile gate:\n{stderr}"
    );
    assert!(!stderr.contains("selected kernel-ir-worker-v2: verified"));
}
