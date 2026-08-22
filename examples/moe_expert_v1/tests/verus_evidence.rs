use std::{path::Path, process::Command};

use fe2o3_moe_expert_v1::MoeExpertExpectedEvidenceV1;
use sha2::{Digest, Sha256};

const PROOF: &[u8] = include_bytes!("../verus/moe_expert_memory_v1.rs");
const KERNEL: &[u8] = include_bytes!("../src/kernel.rs");
const RUNNER: &str = include_str!("../run-verus.sh");
const README: &str = include_str!("../README.md");
const NEGATIVE_MANIFEST: &[u8] = include_bytes!("../verus/NEGATIVE_SHA256");
const CLOSURE_MANIFEST: &[u8] = include_bytes!("../../row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST");
const TRANSCRIPT: &str = "FE2O3_MOE_EXPERT_MEMORY_V1_VERUS_OK mutations=6 obligations=15";

const PROOF_SHA256: &str = "617e6741c5f1415a8e792e5e36e3526c04ba18903438e3af178bb107766383d1";
const KERNEL_SHA256: &str = "5ae3cfe59494347838fe4160c99c5b67968642d26550c01e27d2ee1247511aec";
const RUNNER_SHA256: &str = "140acb3aadc38d59d7e485e7d2044dfed5d03219af16d4fc58c7eeb23c41dc29";
const NEGATIVE_MANIFEST_SHA256: &str =
    "b4690271f253f42bacd387698930064a48f191db5af5743d4cad8ba49084efec";
const CLOSURE_MANIFEST_SHA256: &str =
    "d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019";
const TRANSCRIPT_SHA256: &str = "00e384236423de39f1aadd516a9f40ac1d50645cd1d49d267ff4f7faa47346cc";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn proof_kernel_runner_closure_mutations_and_transcript_are_pinned() {
    assert_eq!(PROOF.len(), 6_720);
    assert_eq!(KERNEL.len(), 6_103);
    assert_eq!(sha256(PROOF), PROOF_SHA256);
    assert_eq!(sha256(KERNEL), KERNEL_SHA256);
    assert_eq!(sha256(RUNNER.as_bytes()), RUNNER_SHA256);
    assert_eq!(sha256(NEGATIVE_MANIFEST), NEGATIVE_MANIFEST_SHA256);
    assert_eq!(sha256(CLOSURE_MANIFEST), CLOSURE_MANIFEST_SHA256);
    assert_eq!(sha256(TRANSCRIPT.as_bytes()), TRANSCRIPT_SHA256);
    assert_eq!(include_str!("../verus/MODEL_SHA256").trim(), PROOF_SHA256);
    assert_eq!(include_str!("../verus/KERNEL_SHA256").trim(), KERNEL_SHA256);
    assert_eq!(
        include_str!("../verus/VERUS_CLOSURE_MANIFEST_SHA256").trim(),
        CLOSURE_MANIFEST_SHA256
    );
    assert_eq!(
        include_str!("../verus/TRANSCRIPT_SHA256").trim(),
        TRANSCRIPT_SHA256
    );
}

#[test]
fn proof_names_bounds_ownership_padding_phase_and_conservative_boundaries() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    for marker in [
        "route_id_is_bounded_v1",
        "token_activation_index_is_bounded_v1",
        "expert_weight_index_is_bounded_v1",
        "expert_tile_index_is_bounded_v1",
        "compact_output_index_is_bounded_v1",
        "combined_output_index_is_bounded_v1",
        "accepted_inverse_value_is_a_compact_slot_v1",
        "padding_rows_are_disjoint_from_active_rows_v1",
        "distinct_expert_tile_coordinates_have_distinct_owners_v1",
        "distinct_compact_coordinates_have_distinct_owners_v1",
        "distinct_combined_coordinates_have_distinct_owners_v1",
        "host_schedule_phase_order_is_exact_v1",
        "compiler_refinement_claimed_v1() -> bool { false }",
        "generalized_machine_memory_safety_claimed_v1() -> bool { false }",
        "generalized_gpu_race_freedom_claimed_v1() -> bool { false }",
        "numerical_correctness_claimed_v1() -> bool { false }",
        "protected_gpu_execution_claimed_v1() -> bool { false }",
    ] {
        assert!(proof.contains(marker), "missing proof marker {marker}");
    }
    for forbidden in ["assume(", "admit(", "external_body", "uninterp spec"] {
        assert!(!proof.contains(forbidden), "proof contains {forbidden}");
    }
}

#[test]
fn all_six_negative_sources_are_pinned_and_executed() {
    let entries: Vec<_> = std::str::from_utf8(NEGATIVE_MANIFEST)
        .unwrap()
        .lines()
        .collect();
    assert_eq!(entries.len(), 6);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("verus");
    for entry in entries {
        let (digest, relative) = entry.split_once("  ").unwrap();
        let source = std::fs::read(root.join(relative)).unwrap();
        assert_eq!(sha256(&source), digest, "mutation drifted: {relative}");
        let stem = Path::new(relative).file_stem().unwrap().to_str().unwrap();
        assert!(RUNNER.contains(stem), "runner omits {relative}");
    }
}

#[test]
fn expected_values_are_exact_but_explicitly_inert() {
    let expected = MoeExpertExpectedEvidenceV1::exact();
    let expected_verus: String = expected
        .verus_executable
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(sha256(PROOF).as_bytes(), PROOF_SHA256.as_bytes());
    assert_eq!(
        expected.proof_source.as_slice(),
        Sha256::digest(PROOF).as_slice()
    );
    assert_eq!(
        expected.kernel_source.as_slice(),
        Sha256::digest(KERNEL).as_slice()
    );
    assert_eq!(
        expected.verus_closure_manifest.as_slice(),
        Sha256::digest(CLOSURE_MANIFEST).as_slice()
    );
    assert_eq!(expected_verus, include_str!("../verus/VERUS_SHA256").trim());
    assert_eq!(
        expected.negative_manifest.as_slice(),
        Sha256::digest(NEGATIVE_MANIFEST).as_slice()
    );
    assert_eq!(
        expected.transcript.as_slice(),
        Sha256::digest(TRANSCRIPT.as_bytes()).as_slice()
    );
    assert!(!expected.authenticates_anything());
}

#[test]
fn runner_is_fail_closed_and_has_no_skip_path() {
    for marker in [
        "verification results:: 15 verified, 0 errors",
        "verification results:: 0 verified, 1 errors",
        "check_digest \"$expected_verus\" \"$verus_path\"",
        "check_digest \"$expected_closure\" \"$closure_manifest\"",
        TRANSCRIPT,
    ] {
        assert!(RUNNER.contains(marker), "runner is missing {marker}");
    }
    for forbidden in ["SKIP:", "exit 77", "VERUS_PROOF", "VERUS_KERNEL"] {
        assert!(!RUNNER.contains(forbidden), "runner contains {forbidden}");
    }
}

#[test]
fn matching_version_fake_verus_is_rejected_by_executable_digest() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(manifest.join("run-verus.sh"))
        .env("VERUS", manifest.join("tests/fixtures/verus"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256 substitution"));
}

#[test]
fn documentation_keeps_the_claim_boundary_explicit() {
    for marker in [
        "ordinary attributed Rust `#[kernel]`",
        "no `macro_rules!`",
        "kernel facade",
        "independent",
        "cannot mint or",
        "join an authenticated receipt",
        "source/model-to-machine refinement",
        "generalized memory safety or race freedom",
    ] {
        assert!(README.contains(marker), "README is missing {marker}");
    }
}

#[test]
#[ignore = "requires the exact pinned Verus closure"]
fn exact_verus_runner_executes_all_evidence() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(manifest.join("run-verus.sh"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(TRANSCRIPT));
}
