use vstd::prelude::*;

verus! {

pub open spec fn tokens_v1() -> nat { 8 }
pub open spec fn experts_v1() -> nat { 4 }
pub open spec fn routes_per_token_v1() -> nat { 2 }
pub open spec fn routes_v1() -> nat { 16 }
pub open spec fn capacity_v1() -> nat { 4 }
pub open spec fn input_width_v1() -> nat { 16 }
pub open spec fn output_width_v1() -> nat { 16 }
pub open spec fn tile_rows_v1() -> nat { 16 }
pub open spec fn tile_elements_v1() -> nat { 256 }
pub open spec fn all_expert_tile_elements_v1() -> nat { 1024 }
pub open spec fn combined_elements_v1() -> nat { 128 }
pub open spec fn drop_route_v1() -> nat { 4294967295 }

pub proof fn exact_shape_is_closed_v1()
    ensures
        tokens_v1() * routes_per_token_v1() == routes_v1(),
        capacity_v1() <= tile_rows_v1(),
        input_width_v1() * output_width_v1() == tile_elements_v1(),
        experts_v1() * tile_elements_v1() == all_expert_tile_elements_v1(),
        tokens_v1() * output_width_v1() == combined_elements_v1(),
{
}

pub open spec fn route_id_v1(token: nat, rank: nat) -> nat {
    token * routes_per_token_v1() + rank
}

pub proof fn route_id_is_bounded_v1(token: nat, rank: nat)
    requires token < tokens_v1(), rank < routes_per_token_v1(),
    ensures route_id_v1(token, rank) < routes_v1(),
{
}

pub open spec fn token_activation_index_v1(token: nat, depth: nat) -> nat {
    token * input_width_v1() + depth
}

pub proof fn token_activation_index_is_bounded_v1(token: nat, depth: nat)
    requires token < tokens_v1(), depth < input_width_v1(),
    ensures token_activation_index_v1(token, depth) < tokens_v1() * input_width_v1(),
{
}

pub open spec fn expert_weight_index_v1(expert: nat, depth: nat, output: nat) -> nat {
    expert * tile_elements_v1() + depth * output_width_v1() + output
}

pub proof fn expert_weight_index_is_bounded_v1(expert: nat, depth: nat, output: nat)
    requires
        expert < experts_v1(),
        depth < input_width_v1(),
        output < output_width_v1(),
    ensures expert_weight_index_v1(expert, depth, output) < all_expert_tile_elements_v1(),
{
}

pub open spec fn expert_tile_index_v1(expert: nat, row: nat, output: nat) -> nat {
    expert * tile_elements_v1() + row * output_width_v1() + output
}

pub proof fn expert_tile_index_is_bounded_v1(expert: nat, row: nat, output: nat)
    requires
        expert < experts_v1(),
        row < tile_rows_v1(),
        output < output_width_v1(),
    ensures expert_tile_index_v1(expert, row, output) < all_expert_tile_elements_v1(),
{
}

pub open spec fn compact_output_index_v1(slot: nat, output: nat) -> nat {
    slot * output_width_v1() + output
}

pub proof fn compact_output_index_is_bounded_v1(slot: nat, output: nat)
    requires slot < routes_v1(), output < output_width_v1(),
    ensures compact_output_index_v1(slot, output) < routes_v1() * output_width_v1(),
{
}

pub open spec fn combined_output_index_v1(token: nat, output: nat) -> nat {
    token * output_width_v1() + output
}

pub proof fn combined_output_index_is_bounded_v1(token: nat, output: nat)
    requires token < tokens_v1(), output < output_width_v1(),
    ensures combined_output_index_v1(token, output) < combined_elements_v1(),
{
}

pub open spec fn valid_inverse_value_v1(slot: nat) -> bool {
    slot < routes_v1() || slot == drop_route_v1()
}

pub proof fn accepted_inverse_value_is_a_compact_slot_v1(slot: nat)
    requires valid_inverse_value_v1(slot), slot != drop_route_v1(),
    ensures slot < routes_v1(),
{
}

pub proof fn active_expert_row_is_bounded_v1(row: nat)
    requires row < capacity_v1(),
    ensures row < tile_rows_v1(),
{
}

pub proof fn padding_rows_are_disjoint_from_active_rows_v1(active: nat, padding: nat)
    requires active < capacity_v1(), capacity_v1() <= padding < tile_rows_v1(),
    ensures active != padding,
{
}

pub proof fn distinct_expert_tile_coordinates_have_distinct_owners_v1(
    left_expert: nat, left_row: nat, left_output: nat,
    right_expert: nat, right_row: nat, right_output: nat,
)
    requires
        left_expert < experts_v1(), right_expert < experts_v1(),
        left_row < tile_rows_v1(), right_row < tile_rows_v1(),
        left_output < output_width_v1(), right_output < output_width_v1(),
        left_expert != right_expert || left_row != right_row || left_output != right_output,
    ensures
        expert_tile_index_v1(left_expert, left_row, left_output)
            != expert_tile_index_v1(right_expert, right_row, right_output),
{
}

pub proof fn distinct_compact_coordinates_have_distinct_owners_v1(
    left_slot: nat, left_output: nat, right_slot: nat, right_output: nat,
)
    requires
        left_slot < routes_v1(), right_slot < routes_v1(),
        left_output < output_width_v1(), right_output < output_width_v1(),
        left_slot != right_slot || left_output != right_output,
    ensures
        compact_output_index_v1(left_slot, left_output)
            != compact_output_index_v1(right_slot, right_output),
{
}

pub proof fn distinct_combined_coordinates_have_distinct_owners_v1(
    left_token: nat, left_output: nat, right_token: nat, right_output: nat,
)
    requires
        left_token < tokens_v1(), right_token < tokens_v1(),
        left_output < output_width_v1(), right_output < output_width_v1(),
        left_token != right_token || left_output != right_output,
    ensures
        combined_output_index_v1(left_token, left_output)
            != combined_output_index_v1(right_token, right_output),
{
}

pub open spec fn compaction_phase_v1() -> nat { 0 }
pub open spec fn expert_gemm_phase_v1() -> nat { 1 }
pub open spec fn inverse_pack_phase_v1() -> nat { 2 }
pub open spec fn combine_phase_v1() -> nat { 3 }

pub proof fn host_schedule_phase_order_is_exact_v1()
    ensures
        compaction_phase_v1() < expert_gemm_phase_v1(),
        expert_gemm_phase_v1() < inverse_pack_phase_v1(),
        inverse_pack_phase_v1() < combine_phase_v1(),
{
}

pub open spec fn compiler_refinement_claimed_v1() -> bool { false }
pub open spec fn logical_address_refinement_claimed_v1() -> bool { false }
pub open spec fn generalized_machine_memory_safety_claimed_v1() -> bool { false }
pub open spec fn generalized_gpu_race_freedom_claimed_v1() -> bool { false }
pub open spec fn numerical_correctness_claimed_v1() -> bool { false }
pub open spec fn protected_gpu_execution_claimed_v1() -> bool { false }

pub proof fn assurance_boundary_is_conservative_v1()
    ensures
        !compiler_refinement_claimed_v1(),
        !logical_address_refinement_claimed_v1(),
        !generalized_machine_memory_safety_claimed_v1(),
        !generalized_gpu_race_freedom_claimed_v1(),
        !numerical_correctness_claimed_v1(),
        !protected_gpu_execution_claimed_v1(),
{
}

} // verus!
