const EDGES_PROOF: &str = include_str!("../verus/lds_tiled_edges_alpha_beta.rs");
const LANE_BARRIER_MUTATION: &str =
    include_str!("../verus/negative/lds_edges_lane_skips_barrier_wrong.rs");
const TAIL_LOAD_MUTATION: &str =
    include_str!("../verus/negative/lds_edges_unguarded_tail_load_wrong.rs");
const TAIL_STORE_MUTATION: &str =
    include_str!("../verus/negative/lds_edges_unguarded_tail_store_wrong.rs");
const ALPHA_BETA_MUTATION: &str = include_str!("../verus/negative/lds_edges_alpha_beta_wrong.rs");
const K_TAIL_MUTATION: &str = include_str!("../verus/negative/lds_edges_k_tail_coverage_wrong.rs");

#[test]
fn edges_proof_source_pins_every_slice4_obligation() {
    for marker in [
        "pub open spec fn bounded_positive_edges_problem_v1",
        "pub open spec fn edges_a_load_enabled_v1",
        "pub open spec fn edges_b_load_enabled_v1",
        "pub open spec fn edges_c_store_enabled_v1",
        "pub proof fn each_lane_predicated_global_load_is_bounded_or_zero_filled_v1",
        "pub proof fn each_lane_predicated_c_access_has_no_oob_store_v1",
        "pub proof fn distinct_valid_edge_output_owners_are_disjoint_v1",
        "pub proof fn each_valid_k_depth_has_exactly_one_tiled_position_v1",
        "pub proof fn valid_k_depth_tiled_position_is_unique_v1",
        "pub proof fn every_oob_tile_element_is_zero_filled_v1",
        "pub proof fn barrier_convergence_is_independent_of_predicates_v1",
        "pub proof fn k_tail_contributes_every_valid_depth_exactly_once_v1",
        "pub proof fn each_valid_edge_output_has_exact_alpha_beta_v1",
    ] {
        assert!(
            EDGES_PROOF.contains(marker),
            "missing proof marker: {marker}"
        );
    }
}

#[test]
fn edges_proofs_and_mutations_contain_no_verifier_shortcuts() {
    for source in [
        EDGES_PROOF,
        LANE_BARRIER_MUTATION,
        TAIL_LOAD_MUTATION,
        TAIL_STORE_MUTATION,
        ALPHA_BETA_MUTATION,
        K_TAIL_MUTATION,
    ] {
        for shortcut in ["admit(", "assume(", "#[verifier::external_body]"] {
            assert!(!source.contains(shortcut), "forbidden shortcut: {shortcut}");
        }
    }
}

#[test]
fn edges_mutations_pin_barrier_bounds_arithmetic_and_tail_failures() {
    assert!(LANE_BARRIER_MUTATION.contains("mutated_predicate_off_lane_still_reaches_barrier_v1"));
    assert!(TAIL_LOAD_MUTATION.contains("mutated_unguarded_tail_load_is_in_bounds_v1"));
    assert!(TAIL_STORE_MUTATION.contains("mutated_unguarded_tail_store_is_in_bounds_v1"));
    assert!(ALPHA_BETA_MUTATION.contains("mutated_wrong_alpha_beta_matches_exact_contract_v1"));
    assert!(K_TAIL_MUTATION.contains("mutated_floor_phases_cover_k_tail_v1"));
}

#[test]
fn edges_model_states_policy_and_evidence_limits() {
    for policy in [
        "edges_empty_output_is_no_dispatch_no_access_v1",
        "edges_legacy_zero_k_fill_reads_no_a_or_b_v1",
        "legacy_zero_k_matches_alpha_beta_only_when_beta_c_is_zero_v1",
    ] {
        assert!(
            EDGES_PROOF.contains(policy),
            "missing policy marker: {policy}"
        );
    }
    for limitation in [
        "source/backend/hardware",
        "IEEE rounding",
        "emitted machine code",
        "hardware behavior",
        "exact real",
        "outside this positive-K milestone",
    ] {
        assert!(
            EDGES_PROOF.contains(limitation),
            "missing limitation: {limitation}"
        );
    }
}
