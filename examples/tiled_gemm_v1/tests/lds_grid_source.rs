const GRID_PROOF: &str = include_str!("../verus/lds_tiled_grid_stride.rs");
const TILE_MAPPING_MUTATION: &str =
    include_str!("../verus/negative/lds_grid_tile_mapping_wrong.rs");
const STRIDE_MUTATION: &str = include_str!("../verus/negative/lds_grid_stride_wrong.rs");
const C_OWNERSHIP_MUTATION: &str = include_str!("../verus/negative/lds_grid_c_ownership_wrong.rs");

#[test]
fn grid_proof_source_pins_every_slice3_obligation() {
    for marker in [
        "pub open spec fn checked_grid_problem_v1",
        "pub open spec fn grid_a_index_v1",
        "pub open spec fn grid_b_index_v1",
        "pub open spec fn grid_c_index_v1",
        "pub proof fn checked_grid_derivation_is_exact_v1",
        "pub proof fn workgroup_to_tile_mapping_is_injective_v1",
        "pub proof fn all_grid_global_a_b_loads_are_in_bounds_v1",
        "pub proof fn each_grid_lane_four_c_stores_are_in_bounds_v1",
        "pub proof fn distinct_grid_invocations_own_disjoint_c_v1",
        "pub proof fn grid_slice1_barrier_converges_for_one_workgroup_v1",
    ] {
        assert!(
            GRID_PROOF.contains(marker),
            "missing proof marker: {marker}"
        );
    }
}

#[test]
fn grid_proofs_and_mutations_contain_no_verifier_shortcuts() {
    for source in [
        GRID_PROOF,
        TILE_MAPPING_MUTATION,
        STRIDE_MUTATION,
        C_OWNERSHIP_MUTATION,
    ] {
        for shortcut in ["admit(", "assume(", "#[verifier::external_body]"] {
            assert!(!source.contains(shortcut), "forbidden shortcut: {shortcut}");
        }
    }
}

#[test]
fn grid_mutations_pin_mapping_stride_and_ownership_failures() {
    assert!(TILE_MAPPING_MUTATION.contains("mutated_grid_mapping_is_injective_v1"));
    assert!(STRIDE_MUTATION.contains("mutated_undersized_lda_keeps_a_load_in_bounds_v1"));
    assert!(C_OWNERSHIP_MUTATION.contains("mutated_distinct_grid_owners_have_disjoint_c_v1"));
}

#[test]
fn grid_model_states_its_fixed_k_and_evidence_limitations() {
    assert!(GRID_PROOF.contains("pub open spec fn grid_k_v1() -> nat { 16 }"));
    for limitation in [
        "kernel-source correspondence",
        "backend refinement",
        "emitted-machine-code safety",
        "hardware behavior",
        "No cross-workgroup barrier",
    ] {
        assert!(
            GRID_PROOF.contains(limitation),
            "missing limitation: {limitation}"
        );
    }
}
