use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

use fe2o3_artifacts::DigestAlgorithm;

const GENUINE_CASES: &[(&str, &str)] = &[
    ("fe2o3-vecadd", "vecadd"),
    ("fe2o3-trusted-item-renamed-genuine", "renamed_genuine"),
];

const REJECTED_CASES: &[(&str, &str, &str)] = &[
    (
        "fe2o3-trusted-item-lookalike-type",
        "lookalike_type",
        "argument 0 has unsupported type",
    ),
    (
        "fe2o3-trusted-item-lookalike-helper",
        "lookalike_helper",
        "missing `output[idx] = <elementwise expression>` store",
    ),
    (
        "fe2o3-trusted-item-lookalike-thread",
        "lookalike_thread",
        "missing `thread::index_1d` call",
    ),
    (
        "fe2o3-trusted-item-external-spoof",
        "external_spoof",
        "MIR is unavailable for a device-reachable item",
    ),
    (
        "fe2o3-trusted-item-local-marker",
        "local_marker",
        "argument 0 has unsupported type",
    ),
    (
        "fe2o3-typed-alias-spoof",
        "typed_alias_spoof",
        "argument 1 must be exactly `&[f32]`",
    ),
];

const REGISTRATION_REJECTED_CASES: &[(&str, &str)] = &[
    (
        "malformed-registration",
        "does not match registration magic",
    ),
    (
        "unknown-registration-version",
        "registration version 3 requires the exact V3 tuple type",
    ),
    (
        "duplicate-logical-name",
        "duplicate logical name `duplicate_logical`",
    ),
    (
        "duplicate-export-name",
        "duplicate export name `duplicate_export`",
    ),
];

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

fn backend_build(workspace: &Path, package: &str) -> Output {
    backend_build_with_args(workspace, package, &[])
}

fn backend_build_with_args(workspace: &Path, package: &str, args: &[&str]) -> Output {
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
            package,
        ])
        .args(args)
        .output()
        .expect("run cargo-fe2o3")
}

fn build_codegen_backend(workspace: &Path) -> PathBuf {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace)
        .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
        .output()
        .expect("build rustc-codegen-fe2o3");
    assert!(
        output.status.success(),
        "backend build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    workspace.join("target/debug/librustc_codegen_fe2o3.so")
}

#[test]
fn local_marker_adversary_clears_generic_frontend_compilation() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let output = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args(["check", "--locked", "-p", "fe2o3-trusted-item-local-marker"])
        .output()
        .expect("check local-marker adversarial fixture");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "local-marker adversary failed before reaching the backend boundary:\n{stderr}"
    );
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn genuine_markers_emit_and_local_external_spoofs_fail_closed() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    for &(package, kernel) in GENUINE_CASES {
        let accepted = backend_build(&workspace, package);
        let stderr = String::from_utf8_lossy(&accepted.stderr);
        assert!(
            accepted.status.success(),
            "genuine device items in `{package}` failed AMDGPU emission:\n{stderr}"
        );
        assert!(
            stderr.contains(&format!("emitted {kernel}")),
            "genuine kernel `{kernel}` did not reach AMDGPU emission:\n{stderr}"
        );
    }

    for &(package, _kernel, expected) in REJECTED_CASES {
        let rejected = backend_build(&workspace, package);
        let stderr = String::from_utf8_lossy(&rejected.stderr);

        assert!(
            !rejected.status.success(),
            "local lookalike package `{package}` unexpectedly emitted AMDGPU code"
        );
        assert!(
            stderr.contains(expected),
            "local lookalike package `{package}` missed `{expected}`:\n{stderr}"
        );
    }

    let duplicate = backend_build_with_args(
        &workspace,
        "fe2o3-trusted-item-local-marker",
        &["--features", "duplicate-genuine"],
    );
    let stderr = String::from_utf8_lossy(&duplicate.stderr);
    assert!(
        !duplicate.status.success(),
        "duplicate semantic marker unexpectedly reached AMDGPU emission"
    );
    assert!(
        stderr.contains("duplicate diagnostic item"),
        "duplicate semantic marker did not fail closed:\n{stderr}"
    );
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn rejected_lookalikes_remove_preseeded_artifacts_atomically() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let artifact_dir = workspace.join("target/fe2o3");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact directory");

    for &(package, kernel, expected) in REJECTED_CASES {
        let artifacts = ["ll", "o", "hsaco"]
            .map(|extension| artifact_dir.join(format!("{kernel}.{extension}")));
        for artifact in &artifacts {
            std::fs::write(artifact, b"preseeded stale artifact")
                .unwrap_or_else(|error| panic!("preseed {}: {error}", artifact.display()));
        }

        let rejected = backend_build(&workspace, package);
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert!(
            !rejected.status.success(),
            "local lookalike package `{package}` unexpectedly emitted AMDGPU code"
        );
        assert!(
            stderr.contains(expected),
            "local lookalike package `{package}` missed `{expected}`:\n{stderr}"
        );
        for artifact in artifacts {
            assert!(
                !artifact.exists(),
                "rejected lookalike left stale artifact {}",
                artifact.display()
            );
        }
    }
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn registration_contract_accepts_genuine_metadata_and_rejects_spoofs_and_duplicates() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let package = "fe2o3-trusted-item-renamed-genuine";

    let prefix_spoof =
        backend_build_with_args(&workspace, package, &["--features", "prefix-spoof"]);
    let stderr = String::from_utf8_lossy(&prefix_spoof.stderr);
    assert!(
        prefix_spoof.status.success(),
        "unregistered prefix spoof should compile as ordinary host code:\n{stderr}"
    );
    assert!(
        !stderr.contains("emitted prefix_spoof"),
        "unregistered prefix spoof unexpectedly reached AMDGPU emission:\n{stderr}"
    );

    for &(feature, expected) in REGISTRATION_REJECTED_CASES {
        let rejected = backend_build_with_args(&workspace, package, &["--features", feature]);
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert!(
            !rejected.status.success(),
            "registration case `{feature}` unexpectedly compiled"
        );
        assert!(
            stderr.contains(expected),
            "registration case `{feature}` missed `{expected}`:\n{stderr}"
        );
    }

    let ordered = backend_build_with_args(&workspace, package, &["--features", "multi-kernel"]);
    let stderr = String::from_utf8_lossy(&ordered.stderr);
    assert!(
        ordered.status.success(),
        "multi-kernel registration build failed:\n{stderr}"
    );
    let alpha = stderr.find("emitted alpha").expect("alpha emission");
    let zeta = stderr.find("emitted zeta").expect("zeta emission");
    assert!(
        alpha < zeta,
        "registered kernels were not emitted in deterministic order:\n{stderr}"
    );
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn general_v3_rejects_local_disjoint_slice_and_index1d_lookalikes() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let target = workspace.join(format!(
        "target/general-v3-spoof-probe-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&target);
    let backend = build_codegen_backend(&workspace);
    let source =
        workspace.join("crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs");
    let worker = std::env::current_exe().expect("current test executable");
    let worker_bytes = std::fs::read(&worker).expect("read current test executable");
    let worker_digest = DigestAlgorithm::Sha256
        .calculate(&worker_bytes)
        .bytes()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    std::fs::create_dir_all(&target).expect("create spoof probe directory");
    let config = target.join("worker-v2-spoof-probe.json");
    let json = format!(
        "{{\"candidate_output_max_bytes\":4194304,\"format\":\"fe2o3-worker-v2-config-v2\",\"limits\":{{\"stderr_bytes\":65536,\"stdout_bytes\":8388608,\"timeout_ms\":30000}},\"link_options\":[{{\"name\":\"code-object-version\",\"value\":\"6\"}},{{\"name\":\"opt-level\",\"value\":\"2\"}},{{\"name\":\"strip-debug\",\"value\":\"true\"}},{{\"name\":\"verify-each\",\"value\":\"true\"}}],\"providers\":[],\"units\":[{{\"crate_name\":\"fe2o3_typed_alias_spoof\",\"source\":{source:?},\"working_directory\":{workspace:?}}}],\"worker\":{{\"byte_len\":{},\"llvm_build_identity\":\"test-only-unreached-llvm\",\"path\":{worker:?},\"sha256\":\"{worker_digest}\",\"worker_build_identity\":\"test-only-unreached-worker\"}}}}",
        worker_bytes.len(),
    );
    std::fs::write(&config, json).expect("write spoof probe Worker V2 config");
    let rejected = Command::new(env!("CARGO"))
        .current_dir(&workspace)
        .args([
            "run",
            "--locked",
            "-p",
            "cargo-fe2o3",
            "--",
            "build",
            "-p",
            "fe2o3-typed-alias-spoof",
            "--features",
            "general-lookalike",
            "--target-dir",
        ])
        .arg(target.join("cargo-target"))
        .env("FE2O3_BACKEND", &backend)
        .env("FE2O3_CODEGEN_PIPELINE", "kernel-ir-worker-v2")
        .env("FE2O3_TARGET", "gfx942:xnack-")
        .env("FE2O3_WORKER_V2_CONFIG_V2", &config)
        .output()
        .expect("run isolated general V3 lookalike probe");
    let _ = std::fs::remove_dir_all(&target);
    let stderr = String::from_utf8_lossy(&rejected.stderr);
    assert!(
        !rejected.status.success(),
        "local general typed lookalikes unexpectedly reached AMDGPU emission"
    );
    assert!(
        stderr.contains("untrusted or unsupported aggregate type"),
        "local general typed lookalike missed the semantic identity rejection:\n{stderr}"
    );
}

#[test]
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn malformed_registrations_invalidate_preseeded_artifacts_atomically() {
    let _lock = backend_test_lock();
    let workspace = workspace();
    let artifact_dir = workspace.join("target/fe2o3");
    std::fs::create_dir_all(&artifact_dir).expect("create artifact directory");

    for &(feature, expected) in &REGISTRATION_REJECTED_CASES[..2] {
        let kernel = feature.replace('-', "_");
        let artifacts = ["ll", "o", "hsaco"]
            .map(|extension| artifact_dir.join(format!("{kernel}.{extension}")));
        for artifact in &artifacts {
            std::fs::write(artifact, b"preseeded stale artifact")
                .unwrap_or_else(|error| panic!("preseed {}: {error}", artifact.display()));
        }

        let rejected = backend_build_with_args(
            &workspace,
            "fe2o3-trusted-item-renamed-genuine",
            &["--features", feature],
        );
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert!(
            !rejected.status.success(),
            "malformed registration `{feature}` unexpectedly compiled"
        );
        assert!(
            stderr.contains(expected),
            "malformed registration `{feature}` missed `{expected}`:\n{stderr}"
        );
        for artifact in artifacts {
            assert!(
                !artifact.exists(),
                "malformed registration left stale artifact {}",
                artifact.display()
            );
        }
    }
}
