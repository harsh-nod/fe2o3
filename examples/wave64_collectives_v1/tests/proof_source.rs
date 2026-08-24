use std::path::{Path, PathBuf};
use std::process::Command;

const PROOF: &str = include_str!("../verus/wave64_collectives_v1.rs");
const REFINEMENT_PROOF: &str = include_str!("../verus/wave64_source_kir_refinement_v1.rs");
const SOURCE_CPU_PROOF: &str =
    include_str!("../verus/wave64_attributed_source_cpu_correspondence_v2.rs");
const ACTIVE_EXCLUSION_WRONG: &str = include_str!("../verus/negative/active_exclusion_wrong.rs");
const BOUNDS_WRONG: &str = include_str!("../verus/negative/bounds_wrong.rs");
const OWNERSHIP_WRONG: &str = include_str!("../verus/negative/ownership_wrong.rs");
const REDUCTION_WRONG: &str = include_str!("../verus/negative/reduction_wrong.rs");
const SCAN_RECURRENCE_WRONG: &str = include_str!("../verus/negative/scan_recurrence_wrong.rs");
const SOURCE_KIR_IDENTITY_WRONG: &str =
    include_str!("../verus/negative/source_kir_identity_wrong.rs");
const SOURCE_KIR_CONTRIBUTOR_WRONG: &str =
    include_str!("../verus/negative/source_kir_contributor_wrong.rs");
const SOURCE_KIR_OWNER_WRONG: &str = include_str!("../verus/negative/source_kir_owner_wrong.rs");
const SOURCE_CPU_MASK_WRONG: &str =
    include_str!("../verus/negative/source_cpu_mask_selection_wrong.rs");
const SOURCE_CPU_SCAN_WRONG: &str =
    include_str!("../verus/negative/source_cpu_scan_order_wrong.rs");
const SOURCE_CPU_ZERO_WRONG: &str =
    include_str!("../verus/negative/source_cpu_inactive_zero_wrong.rs");
const SOURCE_CPU_OWNER_WRONG: &str = include_str!("../verus/negative/source_cpu_owner_wrong.rs");
const SOURCE_CPU_IDENTITY_WRONG: &str =
    include_str!("../verus/negative/source_cpu_correspondence_identity_wrong.rs");
const SOURCE_CPU_OUTER_COMMIT_WRONG: &str =
    include_str!("../verus/negative/source_cpu_outer_commit_wrong.rs");
const RUNNER: &str = include_str!("../run-verus.sh");
const SCANNER: &str = include_str!("../check-proof-source.py");
const README: &str = include_str!("../README.md");

fn example_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[test]
fn proof_names_every_phase_a_obligation() {
    for marker in [
        "pub open spec fn explicit_wave64_mask_v1",
        "mask_bits as nat == mask_prefix_value_v1",
        "pub proof fn active_lane_indices_are_in_bounds_v1",
        "pub proof fn inactive_lane_is_excluded_and_publishes_zero_v1",
        "pub proof fn distinct_lanes_have_injective_output_ownership_v1",
        "pub proof fn masked_reduction_step_recurrence_v1",
        "pub proof fn reduction_is_full_masked_prefix_v1",
        "pub proof fn active_lane_scan_recurrence_v1",
        "pub proof fn empty_mask_has_zero_reduction_and_scans_v1",
    ] {
        assert!(PROOF.contains(marker), "missing formal obligation {marker}");
    }
    assert!(!PROOF.contains("macro_rules!"));
}

#[test]
fn refinement_proof_binds_identity_profile_mask_values_and_ownership() {
    for marker in [
        "pub open spec fn attributed_source_identity_v1",
        "word0: 0x7c6ead1e7c01a61a",
        "pub open spec fn kernel_ir_schema_identity_v1",
        "word0: 0xda2722bd3ce34922",
        "pub open spec fn exact_source_model_to_kernel_ir_profile_v1",
        "target_gfx942_xnack_minus",
        "pub proof fn source_and_kernel_ir_contributors_are_equal_v1",
        "pub proof fn source_and_kernel_ir_prefix_values_are_equal_v1",
        "pub proof fn source_and_kernel_ir_ownership_is_identical_and_injective_v1",
        "pub proof fn exact_masked_reduction_and_scans_refine_semantic_kernel_ir_v1",
        "pub proof fn refinement_boundary_grants_no_adjacent_authority_v1",
    ] {
        assert!(
            REFINEMENT_PROOF.contains(marker),
            "missing refinement obligation {marker}"
        );
    }
}

#[test]
fn source_cpu_proof_binds_reviewed_algorithm_and_keeps_semantic_gap_false() {
    for marker in [
        "pub open spec fn attributed_source_identity_v2",
        "pub open spec fn cpu_oracle_identity_v2",
        "pub open spec fn reviewed_correspondence_identity_v2",
        "pub open spec fn reviewed_outer_public_base_commit_v2",
        "pub proof fn source_and_cpu_select_the_same_active_mask_v2",
        "pub proof fn increasing_lane_recurrences_are_equal_v2",
        "pub proof fn reduction_inclusive_exclusive_and_inactive_publications_match_v2",
        "pub proof fn source_and_cpu_same_lane_ownership_is_equal_and_injective_v2",
        "pub proof fn exact_attributed_source_algorithm_corresponds_to_cpu_oracle_v2",
        "pub open spec fn proves_source_to_model_refinement_v2() -> bool { false }",
        "pub proof fn reviewed_correspondence_grants_no_adjacent_authority_v2",
    ] {
        assert!(
            SOURCE_CPU_PROOF.contains(marker),
            "missing source/CPU obligation {marker}"
        );
    }
}

#[test]
fn expected_negatives_mutate_each_requested_property() {
    for (source, marker) in [
        (
            ACTIVE_EXCLUSION_WRONG,
            "mutated_inactive_lane_contributes_zero_v1",
        ),
        (BOUNDS_WRONG, "mutated_lane_63_output_is_bounded_v1"),
        (
            OWNERSHIP_WRONG,
            "mutated_distinct_lanes_have_distinct_owners_v1",
        ),
        (REDUCTION_WRONG, "mutated_reduction_equals_full_sum_v1"),
        (
            SCAN_RECURRENCE_WRONG,
            "mutated_inclusive_obeys_recurrence_v1",
        ),
        (
            SOURCE_KIR_IDENTITY_WRONG,
            "mutated_kernel_ir_schema_identity_is_exact_v1",
        ),
        (
            SOURCE_KIR_CONTRIBUTOR_WRONG,
            "mutated_exclusive_excludes_its_output_lane_v1",
        ),
        (
            SOURCE_KIR_OWNER_WRONG,
            "mutated_kernel_ir_ownership_is_injective_v1",
        ),
        (
            SOURCE_CPU_MASK_WRONG,
            "mutated_cpu_mask_selection_matches_source_v2",
        ),
        (
            SOURCE_CPU_SCAN_WRONG,
            "mutated_cpu_exclusive_uses_same_physical_prefix_v2",
        ),
        (
            SOURCE_CPU_ZERO_WRONG,
            "mutated_cpu_inactive_publication_is_positive_zero_v2",
        ),
        (
            SOURCE_CPU_OWNER_WRONG,
            "mutated_cpu_owner_matches_same_lane_source_v2",
        ),
        (
            SOURCE_CPU_IDENTITY_WRONG,
            "mutated_reviewed_correspondence_identity_is_exact_v2",
        ),
        (
            SOURCE_CPU_OUTER_COMMIT_WRONG,
            "mutated_outer_public_base_commit_is_exact_v2",
        ),
    ] {
        assert!(source.contains(marker), "missing negative fixture {marker}");
    }
}

#[test]
fn scanner_is_token_aware_and_fail_closed() {
    for marker in [
        "unicodedata.normalize(\"NFKC\"",
        "raw_string_end",
        "unterminated block comment",
        "FORBIDDEN_IDENTIFIERS",
        "conditional proof source is forbidden",
    ] {
        assert!(SCANNER.contains(marker), "missing scanner rule {marker}");
    }

    let scanner = example_root().join("check-proof-source.py");
    let accepted =
        example_root().join("tests/fixtures/source-scanner/accept/comments_and_strings.rs");
    assert!(
        Command::new(&scanner)
            .arg(&accepted)
            .output()
            .unwrap()
            .status
            .success()
    );
    for rejected in ["assume.rs", "external_body.rs", "include.rs"] {
        let output = Command::new(&scanner)
            .arg(
                example_root()
                    .join("tests/fixtures/source-scanner/reject")
                    .join(rejected),
            )
            .output()
            .unwrap();
        assert!(!output.status.success(), "scanner accepted {rejected}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("forbidden proof identifier"));
    }
}

#[test]
fn runner_pins_identity_closure_and_expected_failures() {
    for marker in [
        "VERUS_SHA256",
        "verify-verus-closure.sh",
        "env -i",
        "verification results:: 12 verified, 0 errors",
        "verification results:: 10 verified, 0 errors",
        "verification results:: 13 verified, 0 errors",
        "identity-bound Wave64 source-model-to-Kernel-IR refinement verified",
        "reviewed structural attributed-source-to-CPU correspondence verified",
        "expected-negative proof unexpectedly verified",
        "FE2O3_WAVE64_COLLECTIVES_V1_VERUS_OK",
    ] {
        assert!(RUNNER.contains(marker), "missing runner boundary {marker}");
    }
    assert_eq!(
        include_str!("../verus/VERUS_VERSION"),
        "0.2026.08.02.b677dd5\n"
    );
    assert_eq!(
        include_str!("../verus/VERUS_SHA256"),
        "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd\n"
    );
}

#[test]
fn documentation_keeps_refinement_and_execution_boundaries_explicit() {
    for marker in [
        "Source-model-to-Kernel-IR refinement",
        "Reviewed attributed-source-to-CPU correspondence",
        "7c6ead1e7c01a61a8f31a010c9e8cb9bd1c21a905ba61e9d90c6c077c748ffd4",
        "837aae894e5c04da4b598e45f344f2e5df0aa8bc6155acf0bf05809ecd86d407",
        "d1c8630a5e534fe559db0b669ca55a6f9dda5454a50d57feb67eb3b969941e87",
        "b8daeb2bc953924a424542820bed566e52d57290",
        "da2722bd3ce349228644300b13bb45d4683d1ebd60f8b7749e7764ec6569e894",
        "proves_source_to_model_refinement=false",
        "does not prove semantic source-to-model",
        "does not prove Git-tree membership",
        "does not prove compiler causality",
        "does not prove LLVM/ISA refinement",
        "grants no protected-execution authority",
        "does not establish generalized safety",
        "cannot promote a parity row",
        "No COMGR",
        "in-process LLD library API",
    ] {
        assert!(
            README.contains(marker),
            "missing documentation boundary {marker}"
        );
    }
}
