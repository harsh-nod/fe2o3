use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
            "fe2o3-production-extraction-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create extraction target directory");
        Self { path }
    }
}

impl Drop for ScratchTarget {
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
fn attributed_kernel_is_recollected_inside_a_real_amdgcn_dependency_graph() {
    let target = ScratchTarget::new();
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_extraction_fixture",
        )
        // The production cargo-fe2o3 parent owns this observation. This
        // process-isolation fixture has no production authority and supplies
        // only a nonzero test value so collection can reach the importer gate.
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            "check",
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-extraction-fixture",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(&target.path)
        .output()
        .expect("run AMD extraction fixture");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "importer unexpectedly completed");
    assert!(
        stderr.contains("semantic importer authenticated rustc target \"amdgcn-amd-amdhsa\"")
            && stderr.contains("1 external root(s)")
            && stderr.contains("canonical semantic-MIR construction is not implemented"),
        "missing AMD extraction milestone diagnostic:\n{stderr}"
    );
    for forbidden in [
        "semantic import target rejection",
        "requires authoritative rustc LLVM target",
        "found no registered kernel",
        "legacy-v1",
        "kernel-ir-v1",
    ] {
        assert!(
            !stderr.contains(forbidden),
            "AMD extraction entered forbidden path {forbidden:?}:\n{stderr}"
        );
    }
}
