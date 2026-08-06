use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Mutex, MutexGuard, OnceLock};

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
        "missing `thread::index_1d` call",
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
    ("malformed-registration", "does not match registration magic"),
    (
        "unknown-registration-version",
        "unknown registration version 3",
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
