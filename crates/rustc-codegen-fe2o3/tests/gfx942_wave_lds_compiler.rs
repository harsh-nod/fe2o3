use std::os::unix::ffi::OsStrExt as _;
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

fn cargo_target_root(workspace: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(path) if Path::new(&path).is_absolute() => PathBuf::from(path),
        Some(path) => workspace.join(path),
        None => workspace.join("target"),
    }
}

fn scrub_test_dynamic_loader_environment(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        let bytes = name.as_bytes();
        if bytes.starts_with(b"LD_") || bytes.starts_with(b"DYLD_") || bytes == b"GLIBC_TUNABLES" {
            command.env_remove(name);
        }
    }
}

fn cargo_fe2o3(workspace: &Path) -> &'static Path {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let mut command = Command::new(env!("CARGO"));
            command.current_dir(workspace).args([
                "build",
                "--locked",
                "-p",
                "cargo-fe2o3",
                "--bin",
                "cargo-fe2o3",
            ]);
            scrub_test_dynamic_loader_environment(&mut command);
            let output = command.output().expect("build cargo-fe2o3 test binary");
            assert!(
                output.status.success(),
                "cargo-fe2o3 build failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            cargo_target_root(workspace).join("debug/cargo-fe2o3")
        })
        .as_path()
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
        let source = "crates/rustc-codegen-fe2o3/tests/fixtures/memory-v1-compiler/src/bin/gfx942_wave_lds_v1.rs";
        let path = std::env::temp_dir().join(format!(
            "fe2o3-gfx942-wave-lds-v1-worker-v2-missing-envelope-{}.json",
            std::process::id()
        ));
        let worker = worker.to_str().expect("UTF-8 worker path");
        let working_directory = workspace.to_str().expect("UTF-8 workspace path");
        let json = format!(
            "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-worker-v2-config-v2\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"6\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"units\":[{{\"crate_name\":\"gfx942_wave_lds_v1\",\"source\":{source:?},\"working_directory\":{working_directory:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":\"test-only-unreached-llvm\",\"path\":{worker:?},\"sha256\":\"{digest}\",\"worker_build_identity\":\"test-only-unreached-worker\"}}}}",
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
    let mut command = Command::new(cargo_fe2o3(workspace));
    command
        .current_dir(workspace)
        .args([
            "build",
            "-p",
            "fe2o3-memory-v1-compiler-fixture",
            "--bin",
            "gfx942_wave_lds_v1",
        ])
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env(
            "FE2O3_NON_PRODUCTION_UNPROTECTED_AUTHORITY_VALIDATION_V1",
            "1",
        )
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config.0);
    scrub_test_dynamic_loader_environment(&mut command);
    command
        .output()
        .expect("run gfx942 wave/LDS compiler fixture")
}

#[test]
fn rust_source_reaches_verified_wave_and_lds_kernel_ir() {
    let _lock = backend_test_lock();
    let output = backend_build(&workspace(), "gfx942:xnack-");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "the fixture must stop at the deliberately missing Worker V2 envelope"
    );
    assert!(
        stderr.contains(
            "selected kernel-ir-worker-v2: verified compiler-module candidate with 1 kernel(s)"
        ),
        "the source wave/LDS calls did not reach verified Kernel IR:\n{stderr}"
    );
    assert!(
        stderr.contains("requires a complete compiler FFI envelope"),
        "the fixture missed the expected post-translation Worker V2 boundary:\n{stderr}"
    );
    assert!(!stderr.contains("has no classified trusted device identity"));
    assert!(!stderr.contains("MIR is unavailable for a device-reachable item"));
}

#[test]
fn rust_source_fails_closed_for_every_non_exact_gfx942_xnack_minus_target() {
    let _lock = backend_test_lock();
    for target in [
        "gfx942",
        "gfx942:xnack+",
        "gfx942:sramecc+:xnack-",
        "gfx942:sramecc-:xnack-",
        "gfx942:xnack-:sramecc+",
        "gfx942:xnack-:xnack-",
        "gfx942:xnack-:xnack+",
        "gfx942:future+",
        "gfx941",
        "gfx950",
        "gfx1100",
    ] {
        let output = backend_build(&workspace(), target);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            !output.status.success(),
            "target {target} unexpectedly built"
        );
        assert!(
            stderr.contains("requires exact gfx942:xnack- General V3")
                || stderr.contains("invalid")
                || stderr.contains("unknown")
                || stderr.contains("duplicate")
                || stderr.contains("conflicting"),
            "target {target} missed every exact-target rejection boundary:\n{stderr}"
        );
        assert!(
            !stderr.contains("selected kernel-ir-worker-v2: verified"),
            "target {target} reached verified Kernel IR:\n{stderr}"
        );
    }
}
