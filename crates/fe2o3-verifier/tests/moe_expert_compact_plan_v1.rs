use std::path::Path;

use fe2o3_verifier::{
    MOE_EXPERT_COMPACT_EXPERTS_V1, MOE_EXPERT_COMPACT_OUTPUT_WIDTH_V1,
    MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1, MoeExpertCompactPlanErrorV1,
    MoeExpertCompactPlanExpectedEvidenceV1, check_moe_expert_compact_plan_v1,
};
use sha2::{Digest, Sha256};

const PROOF: &[u8] = include_bytes!("../verus/moe_expert_compact_plan_v1.rs");
const RUNNER: &[u8] = include_bytes!("../../../scripts/test-moe-expert-compact-plan-verus.sh");
const NEGATIVE_MANIFEST: &[u8] =
    include_bytes!("../verus/moe_expert_compact_plan_v1/NEGATIVE_SHA256");
const CLOSURE_MANIFEST: &[u8] =
    include_bytes!("../verus/moe_expert_compact_plan_v1/VERUS_CLOSURE_MANIFEST");
const TRANSCRIPT: &str = "FE2O3_MOE_EXPERT_COMPACT_PLAN_V1_VERUS_OK mutations=7 obligations=19";

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[test]
fn all_625_valid_count_vectors_satisfy_the_exact_compact_plan() {
    let mut checked_vectors = 0;
    for count0 in 0..=4 {
        for count1 in 0..=4 {
            for count2 in 0..=4 {
                for count3 in 0..=4 {
                    let offsets = [
                        0,
                        count0,
                        count0 + count1,
                        count0 + count1 + count2,
                        count0 + count1 + count2 + count3,
                    ];
                    let checked = check_moe_expert_compact_plan_v1(offsets).unwrap();
                    assert_eq!(checked.offsets(), offsets);
                    assert_eq!(checked.accepted_routes(), offsets[4]);
                    assert_eq!(
                        checked.accepted_elements(),
                        offsets[4] * MOE_EXPERT_COMPACT_OUTPUT_WIDTH_V1
                    );
                    assert!(checked.every_source_range_is_inside_its_expert_tile());
                    assert!(checked.every_destination_range_is_inside_compact_tile());
                    assert!(
                        checked.nonempty_destination_ranges_are_pairwise_disjoint_and_ordered()
                    );
                    assert!(checked.destination_union_is_exact_accepted_prefix());
                    checked_vectors += 1;
                }
            }
        }
    }
    assert_eq!(checked_vectors, 625);
}

#[test]
fn malformed_offsets_fail_at_named_boundaries() {
    assert_eq!(
        check_moe_expert_compact_plan_v1([1, 1, 1, 1, 1]),
        Err(MoeExpertCompactPlanErrorV1::FirstOffset)
    );
    assert_eq!(
        check_moe_expert_compact_plan_v1([0, 2, 1, 1, 1]),
        Err(MoeExpertCompactPlanErrorV1::NonMonotone { expert: 1 })
    );
    assert_eq!(
        check_moe_expert_compact_plan_v1([0, 4, 8, 13, 13]),
        Err(MoeExpertCompactPlanErrorV1::Capacity {
            expert: 2,
            count: 5,
        })
    );
}

#[test]
fn zero_fill_preserves_the_prefix_and_defines_the_unused_tail() {
    let checked = check_moe_expert_compact_plan_v1([0, 2, 3, 3, 4]).unwrap();
    let prefix: Vec<_> = (0..checked.accepted_elements() as i32).collect();
    let compact = checked.zero_fill(&prefix).unwrap();
    assert_eq!(&compact[..prefix.len()], prefix.as_slice());
    assert!(compact[prefix.len()..].iter().all(|value| *value == 0));

    assert_eq!(
        checked.zero_fill(&prefix[..prefix.len() - 1]),
        Err(MoeExpertCompactPlanErrorV1::PrefixLength {
            expected: prefix.len(),
            actual: prefix.len() - 1,
        })
    );

    let full = check_moe_expert_compact_plan_v1([0, 4, 8, 12, 16]).unwrap();
    let full_prefix = [7_i32; MOE_EXPERT_COMPACT_TILE_ELEMENTS_V1];
    assert_eq!(full.zero_fill(&full_prefix).unwrap(), full_prefix);
}

#[test]
fn exact_proof_runner_closure_mutations_and_transcript_are_pinned() {
    let expected = MoeExpertCompactPlanExpectedEvidenceV1::exact();
    assert_eq!(sha256(PROOF), expected.proof_source);
    assert_eq!(sha256(RUNNER), expected.runner_source);
    assert_eq!(sha256(NEGATIVE_MANIFEST), expected.negative_manifest);
    assert_eq!(sha256(CLOSURE_MANIFEST), expected.verus_closure_manifest);
    assert_eq!(sha256(TRANSCRIPT.as_bytes()), expected.transcript);
    assert_eq!(
        include_str!("../verus/moe_expert_compact_plan_v1/MODEL_SHA256").trim(),
        hex(&expected.proof_source)
    );
    assert_eq!(
        include_str!("../verus/moe_expert_compact_plan_v1/VERUS_SHA256").trim(),
        hex(&expected.verus_executable)
    );
    assert_eq!(
        include_str!("../verus/moe_expert_compact_plan_v1/VERUS_CLOSURE_MANIFEST_SHA256").trim(),
        hex(&expected.verus_closure_manifest)
    );
    assert_eq!(
        include_str!("../verus/moe_expert_compact_plan_v1/TRANSCRIPT_SHA256").trim(),
        hex(&expected.transcript)
    );
}

#[test]
fn every_named_negative_source_is_pinned_and_executed() {
    let runner = std::str::from_utf8(RUNNER).unwrap();
    let entries: Vec<_> = std::str::from_utf8(NEGATIVE_MANIFEST)
        .unwrap()
        .lines()
        .collect();
    assert_eq!(entries.len(), 7);
    let pin_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("verus/moe_expert_compact_plan_v1");
    for entry in entries {
        let (digest, relative) = entry.split_once("  ").unwrap();
        let source = std::fs::read(pin_dir.join(relative)).unwrap();
        assert_eq!(
            hex(&sha256(&source)),
            digest,
            "mutation drifted: {relative}"
        );
        let stem = Path::new(relative).file_stem().unwrap().to_str().unwrap();
        assert!(runner.contains(stem), "runner omits {relative}");
    }
}

#[test]
fn runner_checks_every_executable_input_without_a_skip_path() {
    let runner = std::str::from_utf8(RUNNER).unwrap();
    for marker in [
        "check_digest \"$expected_model\" \"$proof\"",
        "check_digest \"$expected_verus\" \"$verus_path\"",
        "check_digest \"$expected_closure\" \"$closure_manifest\"",
        "verify-verus-closure.sh",
        "check-proof-source.py",
        "verification results:: 19 verified, 0 errors",
        "verification results:: 0 verified, 1 errors",
        TRANSCRIPT,
    ] {
        assert!(runner.contains(marker), "runner omits pin check: {marker}");
    }
    for forbidden in [
        "allow-unpinned",
        "ALLOW_UNPINNED",
        "skip-verus",
        "SKIP_VERUS",
    ] {
        assert!(!runner.contains(forbidden), "runner contains {forbidden}");
    }
}

#[test]
fn proof_names_the_exact_fixed_profile_and_contains_no_trust_escape() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    for marker in [
        "experts_v1() -> nat { 4 }",
        "capacity_v1() -> nat { 4 }",
        "routes_v1() -> nat { 16 }",
        "output_width_v1() -> nat { 16 }",
        "expert_tile_elements_v1() -> nat { 256 }",
        "each_source_range_lies_inside_its_expert_tile_v1",
        "each_compact_destination_coordinate_is_bounded_v1",
        "nonempty_destination_ranges_are_pairwise_disjoint_and_ordered_v1",
        "destination_union_is_exactly_the_accepted_prefix_v1",
        "zero_fill_defines_every_unused_tail_value_v1",
        "authenticated_proof_receipt_claimed_v1() -> bool { false }",
        "hsa_copy_claimed_v1() -> bool { false }",
        "machine_address_refinement_claimed_v1() -> bool { false }",
        "runtime_execution_claimed_v1() -> bool { false }",
        "gpu_execution_claimed_v1() -> bool { false }",
        "generalized_profile_claimed_v1() -> bool { false }",
    ] {
        assert!(proof.contains(marker), "missing proof marker: {marker}");
    }
    for forbidden in ["assume(", "admit(", "external_body", "uninterp spec"] {
        assert!(!proof.contains(forbidden), "proof contains {forbidden}");
    }
}

#[test]
fn expected_evidence_is_copyable_and_explicitly_inert() {
    let expected = MoeExpertCompactPlanExpectedEvidenceV1::exact();
    let copied = expected;
    assert_eq!(copied, expected);
    assert!(!expected.authenticates_anything());
    assert!(!expected.has_authenticated_proof_receipt());
    assert!(!expected.proves_hsa_copy());
    assert!(!expected.proves_machine_addresses());
    assert!(!expected.proves_runtime_execution());
    assert!(!expected.proves_gpu_execution());
    assert!(!expected.proves_generalized_profile());
    assert_eq!(MOE_EXPERT_COMPACT_EXPERTS_V1, 4);
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
