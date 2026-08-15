use std::process::Command;

const PROOF: &str = include_str!("../verus/workgroup_sync_v1.rs");
const NEGATIVES: [(&str, &str); 6] = [
    (
        "initialization",
        include_str!("../verus/negative/initialization_wrong_slot.rs"),
    ),
    (
        "convergence",
        include_str!("../verus/negative/convergence_divergent_barrier.rs"),
    ),
    (
        "epoch reuse",
        include_str!("../verus/negative/epoch_reuse_missing_barrier.rs"),
    ),
    (
        "ownership",
        include_str!("../verus/negative/ownership_duplicate_writer.rs"),
    ),
    (
        "sum",
        include_str!("../verus/negative/sum_drops_last_lane.rs"),
    ),
    (
        "atomic eligibility",
        include_str!("../verus/negative/atomic_ineligible_contributes.rs"),
    ),
];

#[test]
fn proofs_cover_each_requested_synchronization_surface() {
    for marker in [
        "lane_initializes_its_unique_slot_v1",
        "distinct_lanes_initialize_distinct_slots_v1",
        "all_lanes_reach_one_publish_barrier_v1",
        "epoch_read_is_initialized_and_reuse_is_ordered_v1",
        "lane_zero_is_the_only_output_owner_v1",
        "two_output_owners_are_equal_v1",
        "reduction_step_preserves_exact_sum_v1",
        "complete_reduction_is_exact_prefix_v1",
        "eligible_lane_contributes_once_v1",
        "ineligible_lane_contributes_zero_v1",
        "atomic_final_value_is_initial_plus_eligible_sum_v1",
    ] {
        assert!(PROOF.contains(marker), "missing proof {marker}");
    }
}

#[test]
fn proof_sources_have_no_escape_hatches_and_each_mutation_is_distinct() {
    for (name, source) in std::iter::once(("positive", PROOF)).chain(NEGATIVES) {
        for forbidden in ["admit(", "assume(", "external_body"] {
            assert!(
                !source.contains(forbidden),
                "{name} contains forbidden token {forbidden}"
            );
        }
    }
    for (name, source) in NEGATIVES {
        assert!(source.contains("mutated_"), "{name} lacks a mutation");
    }
}

#[test]
fn pinned_verus_checks_positive_and_expected_negative_proofs_when_available() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(root.join("run-verus.sh"));
    if std::env::var_os("VERUS").is_none() {
        command.env("VERUS", "verus");
    }
    let output = command.output().expect("run pinned Verus helper");
    if output.status.code() == Some(77) {
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return;
    }
    assert!(
        output.status.success(),
        "proof runner failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).expect("proof output is UTF-8");
    assert!(stdout.contains("PASS: pinned workgroup synchronization proofs"));
    assert_eq!(stdout.matches("XFAIL:").count(), 6);
}
