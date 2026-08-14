use vstd::prelude::*;

#[path = "../tiled_gemm_host_contract.rs"]
mod model;

verus! {

/// Mutated A uses the internally injective accumulator orientation.
pub open spec fn mutated_a_row_v1(lane: nat, component: nat) -> nat {
    model::accumulator_row_v1(lane, component)
}

pub open spec fn mutated_a_depth_v1(lane: nat) -> nat {
    model::accumulator_col_v1(lane)
}

pub proof fn mutated_a_mapping_remains_injective_v1(
    left_lane: nat,
    left_component: nat,
    right_lane: nat,
    right_component: nat,
)
    requires
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        left_lane != right_lane || left_component != right_component,
    ensures
        mutated_a_row_v1(left_lane, left_component)
                != mutated_a_row_v1(right_lane, right_component)
            || mutated_a_depth_v1(left_lane) != mutated_a_depth_v1(right_lane),
{
    model::lane_component_register_maps_are_injective_v1(
        left_lane, left_component, right_lane, right_component,
    );
}

/// Expected failure marker: mutated_a_matches_official_table_v1.
pub proof fn mutated_a_matches_official_table_v1(lane: nat, component: nat)
    requires lane < 64, component < 4,
    ensures
        mutated_a_row_v1(lane, component) == model::a_register_row_v1(lane),
        mutated_a_depth_v1(lane) == model::a_register_depth_v1(lane, component),
{
}

} // verus!
