use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

const PROOF: &[u8] = include_bytes!("../verus/moe_top2_v1.rs");
const KERNEL: &[u8] = include_bytes!("../src/kernel.rs");
const RUNNER: &str = include_str!("../run-verus.sh");
const README: &str = include_str!("../README.md");
const NEGATIVE_MANIFEST: &str = include_str!("../verus/NEGATIVE_SHA256");

const PROFILE_IDENTITY: &str = "fe2o3.moe_top2_v1.logits_f32.t8_e4_k2.capacity4.token_major.lower_expert_ties.stable_drop.gfx942_xnack_minus.wave64";
const MODEL_SCHEMA: &str =
    "fe2o3.moe_top2_verus_v1.int_scores.t8_e4_k2_c4.top2_counts_scan_stable_pack_inverse";
const PROOF_SHA256: &str = "4a5a60b66284567522ab3f07d93309c7002abf75870f4aa9db752f8260cb296c";
const KERNEL_SHA256: &str = "0e4570bd52866dd23b8b00d83983aadc818c77580de8f7f5e2982e12a57e20e2";
const PROFILE_SHA256: &str = "4180ef61545684e646bd5227333e7514d22a2d379d7d657397df4d41f7a192d1";
const MODEL_SCHEMA_SHA256: &str =
    "f8543b27093777890dd0d1fab076792421c1d3c64df6571c83c91b3ffa361da7";
const VERUS_VERSION: &str = "0.2026.08.02.b677dd5";
const VERUS_SHA256: &str = "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_proof_source_identity_matches_kernel(proof: &[u8], kernel_sha256: &str) {
    assert_eq!(kernel_sha256.len(), 64);
    let proof = std::str::from_utf8(proof).unwrap();
    let source_identity = proof
        .split_once("pub open spec fn source_identity_v1()")
        .expect("proof source identity")
        .1
        .split_once("\n}")
        .expect("proof source identity terminator")
        .0;
    assert_eq!(source_identity.matches("0x").count(), 4);
    for offset in (0..64).step_by(16) {
        assert!(
            source_identity.contains(&format!("0x{}u64", &kernel_sha256[offset..offset + 16])),
            "proof source identity omits kernel digest limb {}",
            &kernel_sha256[offset..offset + 16]
        );
    }
}

#[test]
fn exact_proof_kernel_profile_and_model_schema_are_identity_bound() {
    assert_eq!(PROOF.len(), 23_108);
    assert_eq!(sha256(PROOF), PROOF_SHA256);
    assert_eq!(sha256(KERNEL), KERNEL_SHA256);
    assert_proof_source_identity_matches_kernel(PROOF, KERNEL_SHA256);
    assert_eq!(sha256(PROFILE_IDENTITY.as_bytes()), PROFILE_SHA256);
    assert_eq!(sha256(MODEL_SCHEMA.as_bytes()), MODEL_SCHEMA_SHA256);

    assert_eq!(include_str!("../verus/MODEL_SHA256").trim(), PROOF_SHA256);
    assert_eq!(include_str!("../verus/KERNEL_SHA256").trim(), KERNEL_SHA256);
    assert_eq!(
        include_str!("../verus/PROFILE_IDENTITY_SHA256").trim(),
        PROFILE_SHA256
    );
    assert_eq!(
        include_str!("../verus/MODEL_SCHEMA_SHA256").trim(),
        MODEL_SCHEMA_SHA256
    );
    assert_eq!(include_str!("../verus/VERUS_VERSION").trim(), VERUS_VERSION);
    assert_eq!(include_str!("../verus/VERUS_SHA256").trim(), VERUS_SHA256);
}

#[test]
fn proof_names_the_complete_obligation_surface() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    let theorem_markers = [
        "exact_evidence_identity_is_admitted_v1",
        "evidence_identity_substitution_fails_closed_v1",
        "lower_expert_id_breaks_equal_score_ties_v1",
        "admitted_top2_has_range_distinctness_and_order_v1",
        "exact_top2_pair_is_deterministic_v1",
        "exact_selection_has_two_ordered_distinct_experts_v1",
        "requested_prefix_is_bounded_v1",
        "admitted_count_relates_request_and_capacity_v1",
        "exclusive_scan_offset_recurrence_v1",
        "exclusive_scan_total_is_bounded_v1",
        "stable_prefix_acceptance_and_drop_v1",
        "accepted_route_slot_is_in_bounds_v1",
        "accepted_route_slots_are_unique_v1",
        "output_counts_capacity_and_scan_are_exact_v1",
        "exact_routing_state_joins_selection_counts_and_packing_v1",
        "accepted_permutation_inverse_round_trip_v1",
        "dropped_routes_and_permutation_tail_are_sentinels_v1",
        "assurance_boundary_is_explicit_v1",
    ];
    for marker in theorem_markers {
        assert!(proof.contains(marker), "missing proof obligation {marker}");
    }
    assert_eq!(
        proof
            .lines()
            .filter(|line| {
                let line = line.trim_start();
                line.starts_with("pub proof fn ") || line.starts_with("proof fn ")
            })
            .count(),
        26
    );
    for boundary in [
        "ieee_f32_refinement_claimed_v1() -> bool { false }",
        "rust_source_refinement_claimed_v1() -> bool { false }",
        "compiler_refinement_claimed_v1() -> bool { false }",
        "machine_safety_claimed_v1() -> bool { false }",
        "gpu_result_claimed_v1() -> bool { false }",
    ] {
        assert!(proof.contains(boundary));
    }
}

#[test]
fn all_nine_negative_mutations_are_exactly_pinned() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries: Vec<_> = NEGATIVE_MANIFEST.lines().collect();
    assert_eq!(entries.len(), 9);
    for entry in entries {
        let (digest, relative) = entry.split_once("  ").unwrap();
        let source = std::fs::read(manifest.join("verus").join(relative)).unwrap();
        assert_eq!(
            sha256(&source),
            digest,
            "negative mutation drifted: {relative}"
        );
        assert!(
            RUNNER.contains(
                relative
                    .trim_start_matches("negative/")
                    .trim_end_matches(".rs")
            )
        );
    }
}

#[test]
fn byte_profile_and_model_substitution_are_rejected_by_pins() {
    let mut proof = PROOF.to_vec();
    let proof_middle = proof.len() / 2;
    proof[proof_middle] ^= 1;
    assert_ne!(sha256(&proof), PROOF_SHA256);

    let mut kernel = KERNEL.to_vec();
    kernel[0] ^= 1;
    assert_ne!(sha256(&kernel), KERNEL_SHA256);

    assert_ne!(
        sha256(PROFILE_IDENTITY.replace("t8_e4", "t8_e8").as_bytes()),
        PROFILE_SHA256
    );
    assert_ne!(
        sha256(MODEL_SCHEMA.replace("int_scores", "f32_scores").as_bytes()),
        MODEL_SCHEMA_SHA256
    );
}

#[test]
fn runner_is_pinned_fail_closed_and_has_no_skip_path() {
    for marker in [
        "check_digest \"$expected_model\" \"$proof\"",
        "check_digest \"$expected_kernel\" \"$kernel\"",
        "sha256_path\" -c NEGATIVE_SHA256",
        "verify-verus-closure.sh",
        "verification results:: 28 verified, 0 errors",
        "verification results:: 0 verified, 1 errors",
        "FE2O3_MOE_TOP2_V1_VERUS_OK",
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
        .env(
            "VERUS",
            manifest.join("tests/fixtures/fake-verus-matching-version.sh"),
        )
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("SHA-256 substitution"), "{stderr}");
}

#[test]
fn documentation_states_the_exact_claim_and_remaining_boundary() {
    for statement in [
        "28 obligations",
        "Nine independently pinned\nmutations",
        "not an IEEE-754 `f32` refinement",
        "a refinement of\n`src/kernel.rs`",
        "a compiler or machine-code refinement",
        "a GPU memory-safety or\ndata-race proof",
        "or a GPU execution result",
    ] {
        assert!(README.contains(statement), "README is missing {statement}");
    }
}
