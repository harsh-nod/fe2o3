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
            "fe2o3-production-ranked-bounds-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create ranked bounds target directory");
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

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn ordinary_rust_bounds_and_production_pliron_pipeline_fail_closed() {
    let safe = run_extraction(&ScratchTarget::new(), false);
    assert!(
        safe.status.success()
            && safe
                .stderr
                .contains("all mandatory kernel checks clean true")
            && safe.stderr.contains("kernel.cond_br")
            && safe.stderr.contains("kernel.access Write")
            && !safe.stderr.contains("error[FE2O3-BOUNDS-001]"),
        "safe checked dynamic access did not pass generic PLIRON verification:\n{}",
        safe.stderr
    );

    let shifted = run_feature_extraction(&ScratchTarget::new(), "shifted");
    assert!(
        shifted.status.success()
            && shifted
                .stderr
                .contains("all mandatory kernel checks clean true")
            && shifted.stderr.contains("kernel.index_binary Add")
            && shifted.stderr.contains("kernel.cond_br")
            && shifted.stderr.contains("kernel.access Write"),
        "safe shifted disjoint access did not pass production extraction:\n{}",
        shifted.stderr,
    );

    let exclusive = run_feature_extraction(&ScratchTarget::new(), "grid_exclusive");
    assert!(
        exclusive.status.success()
            && exclusive
                .stderr
                .contains("all mandatory kernel checks clean true")
            && exclusive.stderr.contains("kernel.index_constant 7")
            && exclusive.stderr.contains("kernel.cond_br")
            && exclusive.stderr.contains("kernel.access Write"),
        "safe grid-exclusive access did not pass production extraction:\n{}",
        exclusive.stderr,
    );

    let blocked = run_feature_extraction(&ScratchTarget::new(), "blocked");
    assert!(
        blocked.status.success()
            && blocked
                .stderr
                .contains("all mandatory kernel checks clean true")
            && blocked.stderr.contains("kernel.index_binary Multiply")
            && blocked.stderr.contains("kernel.index_binary Add")
            && blocked.stderr.contains("kernel.access Write"),
        "safe blocked disjoint access did not pass production extraction:\n{}",
        blocked.stderr,
    );

    let oob = run_extraction(&ScratchTarget::new(), true);
    assert!(
        !oob.status.success(),
        "out-of-bounds Rust kernel was accepted"
    );
    assert!(
        oob.stderr.contains("error[FE2O3-BOUNDS-001]")
            && oob.stderr.contains("required: 64 < 64")
            && oob.stderr.contains("Rust source")
            && oob.stderr.contains(":63:20")
            && oob.stderr.contains("kernel.index_constant 64")
            && oob
                .stderr
                .contains("ranked PLIRON before rejected lowering")
            && oob
                .stderr
                .contains("lowering stopped before target IR or artifact emission"),
        "out-of-bounds diagnostic was incomplete:\n{}",
        oob.stderr,
    );
    for forbidden in ["kernel-ir-v1", "GeneralGemm", "Unknown/Unproved"] {
        assert!(
            !safe.stderr.contains(forbidden) && !oob.stderr.contains(forbidden),
            "production extraction entered forbidden path {forbidden:?}",
        );
    }
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn production_barrier_cfg_preserves_order_and_fails_closed() {
    for feature in ["barrier_after_access", "barrier_before_access"] {
        let output = run_feature_extraction(&ScratchTarget::new(), feature);
        assert!(
            output.status.success()
                && output
                    .stderr
                    .contains("all mandatory kernel checks clean true")
                && output.stderr.contains("kernel.access Write")
                && output.stderr.contains("gpu.barrier"),
            "{feature} did not preserve a clean ranked CFG:\n{}",
            output.stderr,
        );
    }

    for feature in ["barrier_divergent", "barrier_early_return"] {
        let output = run_feature_extraction(&ScratchTarget::new(), feature);
        assert!(
            !output.status.success()
                && output.stderr.contains("error[FE2O3-BARRIER-001]")
                && output.stderr.contains("divergent collective barrier paths"),
            "{feature} did not fail closed as divergent:\n{}",
            output.stderr,
        );
    }

    let cyclic = run_feature_extraction(&ScratchTarget::new(), "barrier_loop");
    assert!(
        !cyclic.status.success()
            && cyclic.stderr.contains("error[FE2O3-BARRIER-002]")
            && cyclic.stderr.contains("cyclic control flow"),
        "cyclic barrier did not remain incomplete:\n{}",
        cyclic.stderr,
    );

    let helper = run_feature_extraction(&ScratchTarget::new(), "barrier_helper");
    assert!(
        !helper.status.success()
            && helper
                .stderr
                .contains(
                    "semantic closure that is neither one kernel root nor one transparent Result wrapper"
                ),
        "helper-mediated barrier bypassed the semantic boundary:\n{}",
        helper.stderr,
    );
}

struct ExtractionOutput {
    status: std::process::ExitStatus,
    stderr: String,
}

fn run_extraction(target: &ScratchTarget, oob: bool) -> ExtractionOutput {
    let mut command = base_command("check", target.path());
    command
        .env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1")
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_ranked_bounds_fixture",
        );
    if oob {
        command.args(["--features", "oob"]);
    }
    output(command, "run AMD extraction fixture")
}

fn run_feature_extraction(target: &ScratchTarget, feature: &str) -> ExtractionOutput {
    let mut command = base_command("check", target.path());
    command
        .env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1")
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_ranked_bounds_fixture",
        )
        .args(["--features", feature]);
    output(command, "run safe mapped AMD extraction fixture")
}

fn base_command(action: &str, target_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "55".repeat(32),
        )
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .args([
            action,
            "--locked",
            "-Zbuild-std=core",
            "-p",
            "fe2o3-production-ranked-bounds-fixture",
            "--target",
            "amdgcn-amd-amdhsa",
            "--target-dir",
        ])
        .arg(target_dir);
    command
}

fn output(mut command: Command, label: &str) -> ExtractionOutput {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    ExtractionOutput {
        status: output.status,
        stderr: String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8"),
    }
}
