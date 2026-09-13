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
fn write_only_reference_preserves_point_mapping_without_read_authority() {
    let target = ScratchTarget::new();
    let positive = run_export_feature(&target.0, "reference-write-only");
    assert!(
        positive.contains("functional-refinement proof runtime unavailable")
            && !positive.contains("has no reference ABI relation"),
        "write-only reference did not reach the existing proof boundary:\n{positive}"
    );
    for feature in [
        "reference-write-only-abi-mismatch",
        "reference-write-only-shared-output",
        "reference-write-only-zero-axes",
        "reference-write-only-two-axes",
        "reference-write-only-slice-output",
    ] {
        let diagnostic = run_export_feature(&target.0, feature);
        assert!(
            diagnostic.contains("logical ABI mismatch at argument 1"),
            "{diagnostic}"
        );
    }
    for feature in [
        "reference-write-only-blocked",
        "reference-write-only-shifted",
        "reference-write-only-custom-space",
    ] {
        let diagnostic = run_export_feature(&target.0, feature);
        assert!(
            diagnostic
                .contains("write-only reference output requires the authenticated Index1D mapping"),
            "{diagnostic}"
        );
    }
    let no_output = run_export_feature(&target.0, "reference-write-only-no-output");
    assert!(
        no_output.contains("no observable output write"),
        "{no_output}"
    );
    let read = run_export_feature(&target.0, "reference-write-only-read");
    assert!(
        read.contains("reference effect scalar operand uses unsupported place projection")
            && !read.contains("functional-refinement proof runtime unavailable"),
        "write-only reference must not assume initialized prior output contents:\n{read}"
    );
}

fn run_export_feature(target: &Path, feature: &str) -> String {
    eprintln!("exporting {feature}");
    let bundle = target.join(format!("{feature}.bundle-v6"));
    let mut command = Command::new("timeout");
    for name in [
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V2",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V3",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V4",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V5",
        "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6",
        "FE2O3_EXTRACT_RANKED_MEMORY_V1",
        "FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
        "FE2O3_EXTRACT_AMDGPU_COMPILER_HANDOFF_PATH_V1",
        "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1",
    ] {
        command.env_remove(name);
    }
    let output = command
        .args(["--kill-after=10s", "300"])
        .arg(env!("CARGO_BIN_EXE_fe2o3-export-sim"))
        .current_dir(workspace())
        .env("CARGO_NET_OFFLINE", "true")
        .args([
            "--crate",
            "fe2o3_production_extraction_fixture",
            "--bundle-version",
            "6",
            "--target",
            "gfx942",
            "--target-dir",
        ])
        .arg(target.join("cargo"))
        .arg("--output")
        .arg(&bundle)
        .args([
            "--",
            "--manifest-path",
            "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/Cargo.toml",
            "--lib",
            "--no-default-features",
            "--features",
            feature,
        ])
        .output()
        .expect("run standard production source exporter");
    let diagnostic = String::from_utf8(output.stderr).expect("UTF-8 compiler diagnostic");
    assert!(
        !output.status.success(),
        "fixture unexpectedly emitted a bundle:\n{diagnostic}"
    );
    assert!(!bundle.exists(), "failed fixture left an output bundle");
    assert!(
        !matches!(output.status.code(), Some(124 | 137)),
        "fixture timed out:\n{diagnostic}"
    );
    diagnostic
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
