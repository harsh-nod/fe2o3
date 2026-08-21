use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const SAFE_PRODUCTION_CRATE_BINDING_V1: &str =
    "9cf5c6d630a1cb5aae7973e2850e5875404c726f813e368ff3d6c53d34bf025c";
const OOB_PRODUCTION_CRATE_BINDING_V1: &str =
    "ed4c8ab709ef6feb1aa913f84536489b3714f591bcf204b5a5414318f6c54289";

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

    let oob = run_extraction(&ScratchTarget::new(), true);
    assert!(
        !oob.status.success(),
        "out-of-bounds Rust kernel was accepted"
    );
    assert!(
        oob.stderr.contains("error[FE2O3-BOUNDS-001]")
            && oob.stderr.contains("required: 64 < 64")
            && oob.stderr.contains("Rust source")
            && oob.stderr.contains(":26:20")
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
    for forbidden in [
        "legacy-v1",
        "kernel-ir-v1",
        "GeneralGemm",
        "Unknown/Unproved",
    ] {
        assert!(
            !safe.stderr.contains(forbidden) && !oob.stderr.contains(forbidden),
            "production extraction entered forbidden path {forbidden:?}",
        );
    }

    let production_target = ScratchTarget::new();
    let safe_production = run_production_pipeline(&production_target, false);
    assert!(
        !safe_production.status.success()
            && safe_production
                .stderr
                .contains("production-v1 target-neutral lowering failed")
            && !safe_production
                .stderr
                .contains("production-v1 general kernel verification failed"),
        "safe kernel did not pass PLIRON before the later lowering boundary:\n{}",
        safe_production.stderr,
    );

    let oob_production = run_production_pipeline(&production_target, true);
    assert!(
        !oob_production.status.success()
            && oob_production
                .stderr
                .contains("production-v1 general kernel verification failed")
            && oob_production.stderr.contains("error[FE2O3-BOUNDS-001]")
            && oob_production.stderr.contains("required: 64 < 64")
            && oob_production.stderr.contains("Rust source")
            && oob_production
                .stderr
                .contains("ranked PLIRON before rejected lowering")
            && oob_production
                .stderr
                .contains("lowering stopped before target IR or artifact emission")
            && !oob_production
                .stderr
                .contains("production-v1 target-neutral lowering failed"),
        "out-of-bounds kernel bypassed the production PLIRON bounds pass:\n{}",
        oob_production.stderr,
    );
    assert!(
        std::fs::read_dir(production_target.path().join("artifacts"))
            .expect("read production artifact directory")
            .next()
            .is_none(),
        "ranked verification emitted a production artifact",
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

fn run_production_pipeline(target: &ScratchTarget, oob: bool) -> ExtractionOutput {
    let artifacts = target.path().join("artifacts");
    std::fs::create_dir_all(&artifacts).expect("create production artifact directory");
    let backend = Path::new(env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
        .parent()
        .expect("extractor binary directory")
        .join(format!(
            "{}rustc_codegen_fe2o3{}",
            std::env::consts::DLL_PREFIX,
            std::env::consts::DLL_SUFFIX,
        ));
    assert!(
        backend.is_file(),
        "missing codegen backend {}",
        backend.display()
    );

    let mut command = base_command("rustc", &target.path().join("cargo"));
    command
        .env("FE2O3_CODEGEN_PIPELINE", "production-v1")
        .env("FE2O3_HSACO_DIR", &artifacts)
        .env("FE2O3_TARGET", "gfx942")
        .env(
            "FE2O3_CRATE_BINDING_ID_V1",
            if oob {
                OOB_PRODUCTION_CRATE_BINDING_V1
            } else {
                SAFE_PRODUCTION_CRATE_BINDING_V1
            },
        );
    if oob {
        command.args(["--features", "oob"]);
    }
    command.args(["--", &format!("-Zcodegen-backend={}", backend.display())]);
    output(command, "run production AMD codegen route")
}

fn base_command(action: &str, target_dir: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .current_dir(workspace())
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "1111111111111111111111111111111111111111111111111111111111111111",
        )
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
