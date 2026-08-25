use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

struct ScratchTarget(PathBuf);

impl ScratchTarget {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-reference-binding-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create reference-binding target");
        Self(path)
    }
}

impl Drop for ScratchTarget {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn run_feature(target: &Path, feature: &str) -> String {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env(
            "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
            "55".repeat(32),
        )
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env(
            "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        )
        .env("FE2O3_EXTRACT_RANKED_MEMORY_V1", "1")
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_extraction_fixture",
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
        .arg(target)
        .args(["--no-default-features", "--features", feature])
        .output()
        .expect("run reference-binding extraction fixture");
    let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
    assert!(
        !output.status.success(),
        "reference fixture {feature} unexpectedly gained artifact authority:\n{stderr}",
    );
    stderr
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn annotated_reference_and_rhs_mutation_are_both_proof_gated_without_runtime() {
    let target = ScratchTarget::new();
    let positive = run_feature(&target.0, "reference-positive");
    assert!(
        positive.contains("functional-refinement proof runtime unavailable at /opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5")
            && positive.contains("compilation stopped before proof admission or artifact emission"),
        "positive reference did not pass the strict effect bijection and reach the proof boundary:\n{positive}",
    );

    let mutated = run_feature(&target.0, "reference-mutated");
    assert!(
        mutated.contains("functional-refinement proof runtime unavailable at /opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5")
            && mutated.contains("compilation stopped before proof admission or artifact emission")
            && !mutated.contains("source-to-proof V2 effect mismatch"),
        "mutated reference did not remain gated by the typed formal-proof boundary:\n{mutated}",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn unsafe_abi_and_unsupported_reference_semantics_fail_closed() {
    let target = ScratchTarget::new();
    for (feature, expected) in [
        ("reference-unsafe", "is declared unsafe"),
        (
            "reference-abi-mismatch",
            "logical ABI mismatch at argument 1",
        ),
        (
            "reference-loop",
            "loops or backedges are outside reference-effect V1",
        ),
        (
            "reference-call",
            "function calls are outside reference-effect V1",
        ),
        (
            "reference-non-function",
            "reference anchor must name exactly one resolvable function item; found 0",
        ),
        (
            "reference-no-output",
            "reference-effect V1 found no observable output write",
        ),
        (
            "reference-duplicate",
            "duplicate safe Rust reference binding for one kernel",
        ),
        (
            "reference-orphan",
            "orphan safe Rust reference binding has no registered kernel",
        ),
        ("reference-generic-mismatch", "type annotations needed"),
        ("reference-missing", "cannot find value"),
    ] {
        let stderr = run_feature(&target.0, feature);
        assert!(
            stderr.contains(expected),
            "reference fixture {feature} lacked precise diagnostic {expected:?}:\n{stderr}",
        );
    }
}
