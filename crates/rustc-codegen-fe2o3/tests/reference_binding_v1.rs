use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const COLLECTED_SHAPE_COMPLETE: &str = "fe2o3 collected-shape: complete; source-proof=not-run; artifact-authority=false; launch-authority=false";
const COLLECTED_ADDRESSES_COMPLETE: &str = "fe2o3 collected-addresses: complete; source-proof=not-run; artifact-authority=false; launch-authority=false";

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

fn run_collected_shape(feature: &str, profile: &str) -> (bool, String) {
    run_collected_observation(feature, profile, false)
}

fn run_collected_addresses(feature: &str, profile: &str) -> (bool, String) {
    run_collected_observation(feature, profile, true)
}

fn run_collected_observation(feature: &str, profile: &str, addresses: bool) -> (bool, String) {
    // A fresh target directory is essential: a Cargo-fresh result did not run
    // this invocation's selected rustc callback.
    let target = ScratchTarget::new();
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(workspace())
        .env("FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2", "55".repeat(32))
        .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS", format!("-Zalways-encode-mir -Ctarget-cpu={profile} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32"))
        .env_remove("FE2O3_EXTRACT_COLLECTED_SHAPE_V1")
        .env_remove("FE2O3_EXTRACT_COLLECTED_ADDRESSES_V1")
        .env("RUSTC_WORKSPACE_WRAPPER", env!("CARGO_BIN_EXE_fe2o3-rustc-extract"))
        .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_production_extraction_fixture");
    for variable in [
        "FE2O3_EXTRACT_RANKED_MEMORY_V1",
        "FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
        "FE2O3_EXTRACT_AMDGPU_COMPILER_HANDOFF_PATH_V1",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V2",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V3",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V4",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V5",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6",
        "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1",
    ] {
        command.env_remove(variable);
    }
    command.env(
        if addresses {
            "FE2O3_EXTRACT_COLLECTED_ADDRESSES_V1"
        } else {
            "FE2O3_EXTRACT_COLLECTED_SHAPE_V1"
        },
        "1",
    );
    let output = command
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
        .arg(&target.0)
        .args(["--no-default-features", "--features", feature])
        .output()
        .expect("run actual collected diagnostic");
    (
        output.status.success(),
        String::from_utf8(output.stderr).expect("UTF-8 diagnostic"),
    )
}

fn require_collected_addresses_success(success: bool, stderr: &str, profile: &str, roots: usize) {
    assert!(
        success,
        "actual collected address relation failed:\n{stderr}"
    );
    assert_eq!(
        stderr
            .lines()
            .filter(|line| *line == COLLECTED_ADDRESSES_COMPLETE)
            .count(),
        1,
        "missing unique address postflight completion:\n{stderr}"
    );
    let profile = match profile {
        "gfx942" => "Gfx942",
        "gfx950" => "Gfx950",
        other => panic!("unexpected test profile {other}"),
    };
    let expected = format!(
        "fe2o3 collected-addresses: incomplete; profile={profile}; references=0; checked-roots={roots}; global-accesses={roots}; source-preservation=conditional-checked; complete-roots={roots}; runtime-premises=undischarged; functional=None; aggregate=absent; source-proof=not-run; artifact-authority=false; launch-authority=false"
    );
    assert_eq!(
        stderr.lines().filter(|line| *line == expected).count(),
        1,
        "actual root/access/None report does not match fixture:\n{stderr}"
    );
    assert!(!stderr.lines().any(|line| line == COLLECTED_SHAPE_COMPLETE));
    assert!(!stderr.contains("functional-refinement proof runtime unavailable"));
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual gfx942/gfx950 collection; no proof runtime"]
fn collected_addresses_checks_all_getmut_roots_on_both_profiles() {
    for profile in ["gfx942", "gfx950"] {
        let (success, stderr) = run_collected_addresses("multi-root-ownership", profile);
        require_collected_addresses_success(success, &stderr, profile, 2);
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD collection; no proof runtime"]
fn collected_addresses_checks_three_root_roster() {
    let (success, stderr) = run_collected_addresses("three-root-ownership", "gfx942");
    require_collected_addresses_success(success, &stderr, "gfx942", 3);
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD collection; no proof runtime"]
fn collected_addresses_retains_nonempty_reference_refusal() {
    for feature in ["reference-positive", "reference-mutated"] {
        let (success, stderr) = run_collected_addresses(feature, "gfx942");
        assert!(
            !success,
            "nonempty original references unexpectedly accepted:\n{stderr}"
        );
        assert!(stderr.contains("collected address diagnostic requires original empty reference bindings; source-proof=not-run"),
            "wrong reference refusal:\n{stderr}");
        assert!(!stderr.contains("functional-refinement proof runtime unavailable"));
        assert!(
            !stderr
                .lines()
                .any(|line| line == COLLECTED_ADDRESSES_COMPLETE)
        );
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD collection; no proof runtime"]
fn collected_addresses_preserves_source_admission_refusals() {
    for (feature, expected) in [
        ("reference-unsafe", "is declared unsafe"),
        (
            "reference-abi-mismatch",
            "logical ABI mismatch at argument 1",
        ),
    ] {
        let (success, stderr) = run_collected_addresses(feature, "gfx942");
        assert!(!success, "source admission unexpectedly passed:\n{stderr}");
        assert!(stderr.contains(expected), "wrong source refusal:\n{stderr}");
        assert!(
            !stderr
                .lines()
                .any(|line| line == COLLECTED_ADDRESSES_COMPLETE)
        );
    }
}

fn require_collected_shape_success(success: bool, stderr: &str) {
    assert!(success, "actual collected shape failed:\n{stderr}");
    assert_eq!(
        stderr
            .lines()
            .filter(|line| *line == COLLECTED_SHAPE_COMPLETE)
            .count(),
        1,
        "missing unique postflight completion:\n{stderr}"
    );
    assert!(stderr.contains("source-proof=not-run"));
    assert!(stderr.contains("references=1"));
    assert!(stderr.contains("ownership 0: ExclusiveOwner"));
    assert!(stderr.contains("DisjointSliceGetMut"));
    assert!(stderr.contains("ThreadIndex1d"));
    assert!(stderr.contains("CallReturn"));
    assert!(stderr.contains("source Store trace 0:"));
    assert_eq!(
        stderr
            .lines()
            .filter(|line| line.starts_with("source Store trace ")
                && line.contains("first-access ordinal=0 matches-traced-store=true: Retained {"))
            .count(),
        1,
        "fixture lacks its exact retained own Store:\n{stderr}"
    );
    for graph in ["N", "B", "O"] {
        assert!(
            stderr
                .lines()
                .any(|line| line.starts_with(&format!("graph {graph} "))
                    && line.contains("uses: Store")),
            "no actual {graph} Store:\n{stderr}"
        );
        assert!(
            stderr
                .lines()
                .any(|line| line.starts_with(&format!("graph {graph} "))
                    && line.contains("terminator:")),
            "no actual {graph} terminator:\n{stderr}"
        );
    }
    assert!(!stderr.contains("functional-refinement proof runtime unavailable"));
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual gfx942/gfx950 extraction; no proof runtime"]
fn collected_reference_shape_observes_actual_source_n_b_o_on_both_profiles() {
    for profile in ["gfx942", "gfx950"] {
        let (success, stderr) = run_collected_shape("reference-positive", profile);
        require_collected_shape_success(success, &stderr);
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD extraction; no proof runtime"]
fn collected_reference_mutation_is_observed_not_authenticated_as_functional_success() {
    let (success, positive) = run_collected_shape("reference-positive", "gfx942");
    require_collected_shape_success(success, &positive);
    let (success, mutated) = run_collected_shape("reference-mutated", "gfx942");
    require_collected_shape_success(success, &mutated);
    let digest = |text: &str| {
        text.lines()
            .find(|line| line.starts_with("source reference 0:"))
            .unwrap()
            .split("effect-sha256=")
            .nth(1)
            .unwrap()
            .split(" writes=")
            .next()
            .unwrap()
            .to_owned()
    };
    assert_ne!(digest(&positive), digest(&mutated));
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD extraction; no proof runtime"]
fn collected_shape_preserves_unsafe_and_abi_source_admission_refusals() {
    for (feature, expected) in [
        ("reference-unsafe", "is declared unsafe"),
        (
            "reference-abi-mismatch",
            "logical ABI mismatch at argument 1",
        ),
    ] {
        let (success, stderr) = run_collected_shape(feature, "gfx942");
        assert!(!success, "source admission unexpectedly passed:\n{stderr}");
        assert!(stderr.contains(expected), "wrong source refusal:\n{stderr}");
        assert!(!stderr.lines().any(|line| line == COLLECTED_SHAPE_COMPLETE));
    }
}

fn run_feature(target: &Path, feature: &str) -> String {
    let output = Command::new(env!("CARGO"))
        .current_dir(workspace())
        .env_remove("FE2O3_EXTRACT_COLLECTED_ADDRESSES_V1")
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
fn annotated_reference_reaches_the_proof_runtime_boundary_and_mutation_is_rejected() {
    let target = ScratchTarget::new();
    let positive = run_feature(&target.0, "reference-positive");
    assert!(
        positive.contains("functional-refinement proof runtime unavailable at /opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5")
            && positive.contains("compilation stopped before proof admission or artifact emission"),
        "positive reference did not pass the strict effect bijection and reach the proof boundary:\n{positive}",
    );

    let mutated = run_feature(&target.0, "reference-mutated");
    assert!(
        (mutated.contains("source-to-proof V2 effect mismatch")
            && mutated.contains("RHS mismatch"))
            || (mutated.contains("functional-refinement proof runtime unavailable")
                && mutated
                    .contains("compilation stopped before proof admission or artifact emission")),
        "mutated reference did not fail closed before artifact authority:\n{mutated}",
    );

    for feature in ["reference-loop", "reference-call", "reference-dynamic-loop"] {
        let stderr = run_feature(&target.0, feature);
        assert!(
            stderr.contains("functional-refinement proof runtime unavailable")
                && !stderr.contains("reference is unsupported"),
            "supported reference feature {feature} did not reach the proof boundary:\n{stderr}",
        );
    }

    let slice_read = run_feature(&target.0, "reference-slice-read");
    assert!(
        ((slice_read.contains("cannot prove full-domain bound")
            && slice_read.contains("no exact ranked extent relation")
            && slice_read.contains("point[0]"))
            || (slice_read.contains("ranked extent %")
                && slice_read.contains("is not an exact constant, argument, or index expression")))
            && !slice_read.contains("functional-refinement proof runtime unavailable"),
        "safe-slice reference gained full-domain authority without an extent proof:\n{slice_read}",
    );
}

#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn two_output_reference_is_joined_once_and_mutations_fail_closed() {
    let target = ScratchTarget::new();
    let positive = run_feature(&target.0, "reference-two-output-positive");
    assert!(
        positive.contains("functional-refinement proof runtime unavailable")
            && !positive.contains("requires exactly one observable reference output write"),
        "two-output reference did not complete the compiler-owned join before proof execution:\n{positive}",
    );

    let substitution = run_feature(&target.0, "reference-two-output-substitution");
    assert!(
        substitution.contains("functional-refinement proof runtime unavailable")
            || substitution.contains("source-to-proof V2 effect mismatch")
            || substitution.contains("functional-refinement proof execution failed"),
        "two-output RHS substitution did not reach an authenticated rejection boundary:\n{substitution}",
    );

    let alias = run_feature(&target.0, "reference-two-output-alias");
    assert!(
        alias.contains("source-to-proof V2 effect mismatch")
            && (alias.contains("has no GPU output effect")
                || alias.contains("multiple indistinguishable GPU output effects")),
        "two-output alias mutation was not rejected by the live effect bijection:\n{alias}",
    );

    let schedule = run_feature(&target.0, "reference-two-output-schedule");
    assert!(
        schedule.contains("logical path guard outside the exact memory-bounds selection")
            || schedule.contains("guard mismatch"),
        "two-output schedule mutation was not rejected before proof admission:\n{schedule}",
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
            "reference-nested-call",
            "nested safe helper calls are unsupported",
        ),
        (
            "reference-helper-memory",
            "outside pure scalar helper summaries",
        ),
        (
            "reference-helper-unsafe",
            "contains a user-provided unsafe block",
        ),
        (
            "reference-helper-recursive",
            "recursive safe scalar helper is unsupported",
        ),
        ("reference-loop-overflow", "is statically false"),
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
