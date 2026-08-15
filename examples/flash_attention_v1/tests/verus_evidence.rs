use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

const PROOF: &[u8] = include_bytes!("../verus/flash_attention_v1.rs");
const KERNEL: &[u8] = include_bytes!("../src/kernel.rs");
const SOURCE_CHECKER: &[u8] = include_bytes!("../verus/check-proof-source.py");
const CLOSURE_MANIFEST: &[u8] = include_bytes!("../../row_softmax_v1/verus/VERUS_CLOSURE_MANIFEST");
const RUNNER: &str = include_str!("../run-verus.sh");
const README: &str = include_str!("../README.md");
const NEGATIVE_MANIFEST: &str = include_str!("../verus/NEGATIVE_SHA256");

const PROFILE_IDENTITY: &str = "fe2o3.flash_attention_v1.causal.qkv_f32.b1_h1_n8_d16.row_major.scale_0p25.gfx942_xnack_minus.wave64";
const MODEL_SCHEMA: &str = "fe2o3.flash_attention_verus_v1.exact_rational.b1_h1_n8_d16.causal_online_max_rescale_sum_numerator_output_ownership";
const PROOF_SHA256: &str = "e1f48bb3dc7bee0678898d13660bf4ce02d9d8e5706e3969f11b11c8b1d7a2da";
const KERNEL_SHA256: &str = "2b00a64e43e69c416e70080e013edf90e861fef94ee66441da93d2c11b3e8f17";
const PROFILE_SHA256: &str = "4dfe870bb76dd32b49144ee70ec4925eab8677b7cbd1a1bfe99fa2294f85fec8";
const MODEL_SCHEMA_SHA256: &str =
    "f26a435e375adfeb1753dd7429870532b90c88bbd46054b9498c82408bcd062b";
const SOURCE_CHECKER_SHA256: &str =
    "a2cf9bebabb0a95b0b8c23586b1fe120a3d8571d9d7809be8ed9fdd2a035d531";
const CLOSURE_MANIFEST_SHA256: &str =
    "d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019";
const VERUS_VERSION: &str = "0.2026.08.02.b677dd5";
const VERUS_SHA256: &str = "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn exact_proof_kernel_profile_model_and_tooling_are_identity_bound() {
    assert_eq!(PROOF.len(), 19_309);
    assert_eq!(sha256(PROOF), PROOF_SHA256);
    assert_eq!(sha256(KERNEL), KERNEL_SHA256);
    assert_eq!(sha256(PROFILE_IDENTITY.as_bytes()), PROFILE_SHA256);
    assert_eq!(sha256(MODEL_SCHEMA.as_bytes()), MODEL_SCHEMA_SHA256);
    assert_eq!(sha256(SOURCE_CHECKER), SOURCE_CHECKER_SHA256);
    assert_eq!(sha256(CLOSURE_MANIFEST), CLOSURE_MANIFEST_SHA256);

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
    assert_eq!(
        include_str!("../verus/SOURCE_CHECKER_SHA256").trim(),
        SOURCE_CHECKER_SHA256
    );
    assert_eq!(
        include_str!("../verus/VERUS_CLOSURE_MANIFEST_SHA256").trim(),
        CLOSURE_MANIFEST_SHA256
    );
    assert_eq!(include_str!("../verus/VERUS_VERSION").trim(), VERUS_VERSION);
    assert_eq!(include_str!("../verus/VERUS_SHA256").trim(), VERUS_SHA256);
}

#[test]
fn proof_names_the_complete_flash_attention_obligation_surface() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    let theorem_markers = [
        "exact_evidence_identity_is_admitted_v1",
        "evidence_identity_substitution_fails_closed_v1",
        "exact_profile_dimensions_and_extent_v1",
        "causal_domain_is_nonempty_and_bounded_v1",
        "future_keys_are_excluded_v1",
        "tensor_index_is_bounded_v1",
        "causal_qkv_indices_are_bounded_v1",
        "distinct_lane_slots_have_distinct_outputs_v1",
        "every_output_has_exact_owner_v1",
        "initial_online_state_is_exact_v1",
        "maximum_frame_update_bounds_both_v1",
        "online_denominator_is_nonzero_v1",
        "online_step_preserves_sum_and_numerator_v1",
        "online_state_matches_causal_reference_v1",
        "exact_profile_output_cell_is_owned_and_bounded_v1",
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
        22
    );
}

#[test]
fn exponential_abstraction_and_withheld_authorities_are_explicit() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    assert_eq!(proof.matches("pub uninterp spec fn ").count(), 1);
    assert!(proof.contains("pub uninterp spec fn exp_weight_v1"));
    for boundary in [
        "exponential_numerical_law_claimed_v1() -> bool { false }",
        "ieee_f32_refinement_claimed_v1() -> bool { false }",
        "ocml_refinement_claimed_v1() -> bool { false }",
        "rust_source_refinement_claimed_v1() -> bool { false }",
        "compiler_kir_refinement_claimed_v1() -> bool { false }",
        "llvm_isa_refinement_claimed_v1() -> bool { false }",
        "machine_safety_claimed_v1() -> bool { false }",
        "data_race_freedom_claimed_v1() -> bool { false }",
        "gpu_result_claimed_v1() -> bool { false }",
    ] {
        assert!(proof.contains(boundary), "missing boundary {boundary}");
    }
    for forbidden in ["assume(", "admit(", "external_body"] {
        assert!(!proof.contains(forbidden), "proof contains {forbidden}");
    }
}

#[test]
fn all_ten_negative_mutations_are_exactly_pinned() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries: Vec<_> = NEGATIVE_MANIFEST.lines().collect();
    assert_eq!(entries.len(), 10);
    for entry in entries {
        let (digest, relative) = entry.split_once("  ").unwrap();
        let source = std::fs::read(manifest.join("verus").join(relative)).unwrap();
        assert_eq!(sha256(&source), digest, "mutation drifted: {relative}");
        assert!(
            RUNNER.contains(
                relative
                    .trim_start_matches("negative/")
                    .trim_end_matches(".rs")
            ),
            "runner omits {relative}"
        );
    }
}

#[test]
fn proof_kernel_profile_and_model_substitutions_are_rejected_by_pins() {
    let mut proof = PROOF.to_vec();
    let middle = proof.len() / 2;
    proof[middle] ^= 1;
    assert_ne!(sha256(&proof), PROOF_SHA256);

    let mut kernel = KERNEL.to_vec();
    kernel[0] ^= 1;
    assert_ne!(sha256(&kernel), KERNEL_SHA256);

    assert_ne!(
        sha256(PROFILE_IDENTITY.replace("n8_d16", "n16_d16").as_bytes()),
        PROFILE_SHA256
    );
    assert_ne!(
        sha256(
            MODEL_SCHEMA
                .replace("exact_rational", "ieee_f32")
                .as_bytes()
        ),
        MODEL_SCHEMA_SHA256
    );
}

#[test]
fn runner_is_pinned_fail_closed_and_has_no_skip_path() {
    for marker in [
        "check_digest \"$expected_model\" \"$proof\"",
        "check_digest \"$expected_kernel\" \"$kernel\"",
        "check_digest \"$expected_checker\" \"$source_checker\"",
        "check_digest \"$expected_closure\" \"$closure_manifest\"",
        "sha256_path\" -c NEGATIVE_SHA256",
        "verification results:: 25 verified, 0 errors",
        "verification results:: 0 verified, 1 errors",
        "FE2O3_FLASH_ATTENTION_V1_VERUS_OK mutations=10 obligations=25",
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
        "25 obligations",
        "Ten independently pinned mutations",
        "sole uninterpreted transcendental abstraction",
        "proves no exponential law",
        "not an IEEE-754 `f32` or OCML numerical refinement",
        "refinement of `src/kernel.rs`",
        "a compiler/Kernel-IR/LLVM/ISA refinement",
        "machine-safety proof",
        "a GPU data-race-freedom proof",
        "or a GPU execution result",
    ] {
        assert!(README.contains(statement), "README is missing {statement}");
    }
}
