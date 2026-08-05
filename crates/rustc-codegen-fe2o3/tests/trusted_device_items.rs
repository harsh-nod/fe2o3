use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
];

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
#[ignore = "requires the configured ROCm LLVM toolchain"]
fn genuine_markers_emit_and_local_external_spoofs_fail_closed() {
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
