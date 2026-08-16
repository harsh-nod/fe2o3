use std::{path::Path, process::Command};

use sha2::{Digest, Sha256};

const PROOF: &[u8] = include_bytes!("../verus/moe_top2_memory_v1.rs");
const KERNEL: &[u8] = include_bytes!("../src/kernel.rs");
const RUNNER: &str = include_str!("../run-memory-verus.sh");
const README: &str = include_str!("../README.md");
const CLOSURE: &[u8] = include_bytes!("../verus/MEMORY_VERUS_CLOSURE_MANIFEST");
const NEGATIVES: &str = include_str!("../verus/MEMORY_NEGATIVE_SHA256");

const PROOF_SHA256: &str = "a17fad7c3f774ba5d2756505a65173350b6706c5fa209e76556383ceed4a2ac9";
const KERNEL_SHA256: &str = "b77016caa0c3708e420e583712e65e4e6428db7b4feafd8d0a1d4bdc475ef6ff";
const CLOSURE_SHA256: &str = "f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01";
const TRANSCRIPT: &str = "FE2O3_MOE_TOP2_MEMORY_V1_VERUS_OK mutations=8 obligations=16";
const TRANSCRIPT_SHA256: &str = "6344a0def7204969b6218f7e81a4edfb65f21fcb272bfd6af1db19917c46c3b9";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn bounded_memory_proof_source_and_execution_closure_are_pinned() {
    assert_eq!(PROOF.len(), 12_545);
    assert_eq!(sha256(PROOF), PROOF_SHA256);
    assert_eq!(sha256(KERNEL), KERNEL_SHA256);
    assert_eq!(sha256(CLOSURE), CLOSURE_SHA256);
    assert_eq!(sha256(TRANSCRIPT.as_bytes()), TRANSCRIPT_SHA256);
    assert_eq!(
        include_str!("../verus/MEMORY_MODEL_SHA256").trim(),
        PROOF_SHA256
    );
    assert_eq!(
        include_str!("../verus/MEMORY_VERUS_CLOSURE_MANIFEST_SHA256").trim(),
        CLOSURE_SHA256
    );
    assert_eq!(
        include_str!("../verus/MEMORY_TRANSCRIPT_SHA256").trim(),
        TRANSCRIPT_SHA256
    );
}

#[test]
fn proof_names_all_eight_buffers_ownership_and_conservative_boundaries() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    for marker in [
        "exact_eight_buffer_extents_v1",
        "token_logit_index_is_bounded_v1",
        "token_rank_route_id_is_bounded_v1",
        "route_slot_permutation_and_inverse_values_are_bounded_v1",
        "every_exact_abi_access_is_in_bounds_v1",
        "pairwise_disjoint_regions_have_distinct_element_addresses_v1",
        "distinct_output_elements_have_distinct_write_owners_v1",
        "no_duplicate_external_write_ownership_v1",
        "stable_routing_phases_precede_output_commit_v1",
        "published_machine_body_identity_v1",
        "analyzer_profile_identity_v1",
        "compiler_refinement_claimed_v1() -> bool { false }",
        "kernel_ir_refinement_claimed_v1() -> bool { false }",
        "logical_address_refinement_claimed_v1() -> bool { false }",
        "isa_refinement_claimed_v1() -> bool { false }",
        "artifact_authority_claimed_v1() -> bool { false }",
        "generalized_machine_memory_safety_claimed_v1() -> bool { false }",
        "generalized_gpu_race_freedom_claimed_v1() -> bool { false }",
        "gpu_execution_claimed_v1() -> bool { false }",
    ] {
        assert!(proof.contains(marker), "missing proof boundary {marker}");
    }
    for forbidden in ["assume(", "admit(", "external_body", "uninterp spec"] {
        assert!(!proof.contains(forbidden), "proof contains {forbidden}");
    }
}

#[test]
fn all_eight_verus_mutations_are_pinned_and_executed() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries: Vec<_> = NEGATIVES.lines().collect();
    assert_eq!(entries.len(), 8);
    for entry in entries {
        let (digest, relative) = entry.split_once("  ").unwrap();
        let source = std::fs::read(manifest.join("verus").join(relative)).unwrap();
        assert_eq!(sha256(&source), digest, "mutation drifted: {relative}");
        let stem = Path::new(relative).file_stem().unwrap().to_str().unwrap();
        assert!(RUNNER.contains(stem), "runner omits {relative}");
    }
}

#[test]
fn runner_is_fail_closed_and_transcript_is_pinned() {
    for marker in [
        "verification results:: 16 verified, 0 errors",
        "verification results:: 0 verified, 1 errors",
        "check_digest \"$expected_proof\" \"$proof\"",
        "check_digest \"$expected_verus\" \"$verus_path\"",
        "\"$closure_checker\" \"$verus_root\" \"$closure_manifest\"",
        "actual_transcript=$(printf '%s' \"$transcript\"",
        TRANSCRIPT,
    ] {
        assert!(RUNNER.contains(marker), "runner is missing {marker}");
    }
    for forbidden in ["SKIP:", "exit 77", "VERUS_PROOF", "VERUS_KERNEL"] {
        assert!(!RUNNER.contains(forbidden), "runner contains {forbidden}");
    }
}

#[test]
fn evidence_mutations_change_their_expected_identities() {
    let mut proof = PROOF.to_vec();
    proof[PROOF.len() / 2] ^= 1;
    assert_ne!(sha256(&proof), PROOF_SHA256);
    assert_ne!(
        sha256(
            TRANSCRIPT
                .replace("obligations=16", "obligations=15")
                .as_bytes()
        ),
        TRANSCRIPT_SHA256
    );
}

#[test]
fn documentation_keeps_the_finite_logical_claim_boundary_explicit() {
    for marker in [
        "finite logical-source proof",
        "copyable and inert and authenticates nothing",
        "does not mint or join an",
        "source/compiler/KIR/LLVM/ISA",
        "generalized\nmachine memory safety or GPU race freedom",
        "or report GPU execution",
    ] {
        assert!(README.contains(marker), "README is missing {marker}");
    }
}

#[test]
fn matching_version_fake_verus_is_rejected_by_executable_digest() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(manifest.join("run-memory-verus.sh"))
        .env("VERUS", manifest.join("tests/fixtures/verus"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("SHA-256 substitution"));
}

#[test]
#[ignore = "requires the exact pinned Verus closure"]
fn exact_memory_verus_runner_executes_pinned_evidence() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(manifest.join("run-memory-verus.sh"))
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
