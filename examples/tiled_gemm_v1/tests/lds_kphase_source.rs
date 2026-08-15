const PROOF_SOURCE: &str = include_str!("../verus/lds_tiled_kphase.rs");
const REUSE_MUTATION: &str = include_str!("../verus/negative/lds_kphase_reuse_wrong.rs");
const RESET_MUTATION: &str =
    include_str!("../verus/negative/lds_kphase_accumulator_reset_wrong.rs");

#[test]
fn kphase_proof_source_pins_every_slice2_obligation() {
    for marker in [
        "pub open spec fn kphase_write_epoch_v1",
        "pub open spec fn kphase_read_epoch_v1",
        "pub open spec fn kphase_reuse_epoch_v1",
        "pub proof fn bounded_kphase_global_loads_v1",
        "pub proof fn bounded_k_phases_partition_depth_v1",
        "pub proof fn every_kphase_a_read_is_initialized_v1",
        "pub proof fn every_kphase_b_read_is_initialized_v1",
        "pub proof fn kphase_publish_and_reuse_barriers_converge_v1",
        "pub proof fn no_kphase_overwrite_before_prior_reads_v1",
        "pub proof fn kphase_inner_accumulator_invariant_v1",
        "pub proof fn kphase_accumulator_invariant_preserved_v1",
        "pub proof fn kphase_final_c_stores_are_disjoint_v1",
        "pub proof fn bounded_kphase_lds_result_is_matrix_product_v1",
    ] {
        assert!(
            PROOF_SOURCE.contains(marker),
            "missing proof marker: {marker}"
        );
    }
}

#[test]
fn kphase_proofs_and_mutations_contain_no_verifier_shortcuts() {
    for source in [PROOF_SOURCE, REUSE_MUTATION, RESET_MUTATION] {
        for shortcut in ["admit(", "assume(", "#[verifier::external_body]"] {
            assert!(!source.contains(shortcut), "forbidden shortcut: {shortcut}");
        }
    }
}

#[test]
fn kphase_mutations_pin_reuse_and_accumulator_failures() {
    assert!(REUSE_MUTATION.contains("mutated_missing_reuse_epoch_protects_prior_reads_v1"));
    assert!(RESET_MUTATION.contains("mutated_accumulator_reset_preserves_k_product_v1"));
}
