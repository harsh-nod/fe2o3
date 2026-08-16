use vstd::prelude::*;

verus! {

pub open spec fn experts_v1() -> nat { 4 }
pub open spec fn capacity_v1() -> nat { 4 }
pub open spec fn routes_v1() -> nat { 16 }
pub open spec fn output_width_v1() -> nat { 16 }
pub open spec fn expert_tile_elements_v1() -> nat { 256 }
pub open spec fn compact_tile_elements_v1() -> nat { 256 }

pub open spec fn valid_offsets_v1(offsets: Seq<nat>) -> bool {
    &&& offsets.len() == experts_v1() + 1
    &&& offsets[0] == 0
    &&& offsets[0] <= offsets[1]
    &&& offsets[1] <= offsets[2]
    &&& offsets[2] <= offsets[3]
    &&& offsets[3] <= offsets[4]
    &&& offsets[1] - offsets[0] <= capacity_v1()
    &&& offsets[2] - offsets[1] <= capacity_v1()
    &&& offsets[3] - offsets[2] <= capacity_v1()
    &&& offsets[4] - offsets[3] <= capacity_v1()
}

pub open spec fn expert_count_v1(offsets: Seq<nat>, expert: nat) -> nat
    recommends valid_offsets_v1(offsets), expert < experts_v1(),
{
    (offsets[(expert + 1) as int] - offsets[expert as int]) as nat
}

pub open spec fn expert_tile_start_v1(expert: nat) -> nat {
    expert * expert_tile_elements_v1()
}

pub open spec fn expert_tile_end_v1(expert: nat) -> nat {
    (expert + 1) * expert_tile_elements_v1()
}

pub open spec fn source_range_start_v1(expert: nat) -> nat {
    expert_tile_start_v1(expert)
}

pub open spec fn source_range_end_v1(offsets: Seq<nat>, expert: nat) -> nat
    recommends valid_offsets_v1(offsets), expert < experts_v1(),
{
    source_range_start_v1(expert) + expert_count_v1(offsets, expert) * output_width_v1()
}

pub open spec fn source_index_v1(offsets: Seq<nat>, expert: nat, row: nat, column: nat) -> nat
    recommends
        valid_offsets_v1(offsets),
        expert < experts_v1(),
        row < expert_count_v1(offsets, expert),
        column < output_width_v1(),
{
    source_range_start_v1(expert) + row * output_width_v1() + column
}

pub open spec fn destination_range_start_v1(offsets: Seq<nat>, expert: nat) -> nat
    recommends valid_offsets_v1(offsets), expert < experts_v1(),
{
    offsets[expert as int] * output_width_v1()
}

pub open spec fn destination_range_end_v1(offsets: Seq<nat>, expert: nat) -> nat
    recommends valid_offsets_v1(offsets), expert < experts_v1(),
{
    offsets[(expert + 1) as int] * output_width_v1()
}

pub open spec fn destination_index_v1(
    offsets: Seq<nat>,
    expert: nat,
    row: nat,
    column: nat,
) -> nat
    recommends
        valid_offsets_v1(offsets),
        expert < experts_v1(),
        row < expert_count_v1(offsets, expert),
        column < output_width_v1(),
{
    destination_range_start_v1(offsets, expert) + row * output_width_v1() + column
}

pub open spec fn accepted_prefix_end_v1(offsets: Seq<nat>) -> nat
    recommends valid_offsets_v1(offsets),
{
    offsets[4] * output_width_v1()
}

pub open spec fn in_destination_range_v1(offsets: Seq<nat>, expert: nat, index: nat) -> bool
    recommends valid_offsets_v1(offsets), expert < experts_v1(),
{
    destination_range_start_v1(offsets, expert) <= index
        && index < destination_range_end_v1(offsets, expert)
}

pub open spec fn in_destination_union_v1(offsets: Seq<nat>, index: nat) -> bool
    recommends valid_offsets_v1(offsets),
{
    ||| in_destination_range_v1(offsets, 0, index)
    ||| in_destination_range_v1(offsets, 1, index)
    ||| in_destination_range_v1(offsets, 2, index)
    ||| in_destination_range_v1(offsets, 3, index)
}

pub open spec fn zero_filled_value_v1(
    offsets: Seq<nat>,
    accepted_values: Seq<int>,
    index: nat,
) -> int
    recommends
        valid_offsets_v1(offsets),
        accepted_values.len() == accepted_prefix_end_v1(offsets),
        index < compact_tile_elements_v1(),
{
    if index < accepted_prefix_end_v1(offsets) {
        accepted_values[index as int]
    } else {
        0
    }
}

pub proof fn exact_compact_shape_is_closed_v1()
    ensures
        experts_v1() * capacity_v1() == routes_v1(),
        routes_v1() * output_width_v1() == compact_tile_elements_v1(),
        capacity_v1() * output_width_v1() <= expert_tile_elements_v1(),
{
}

pub proof fn valid_offsets_are_route_bounded_v1(offsets: Seq<nat>)
    requires valid_offsets_v1(offsets),
    ensures
        offsets[1] <= 4,
        offsets[2] <= 8,
        offsets[3] <= 12,
        offsets[4] <= routes_v1(),
{
}

pub proof fn every_expert_count_is_capacity_bounded_v1(offsets: Seq<nat>, expert: nat)
    requires valid_offsets_v1(offsets), expert < experts_v1(),
    ensures expert_count_v1(offsets, expert) <= capacity_v1(),
{
    if expert == 0 {
    } else if expert == 1 {
    } else if expert == 2 {
    } else {
        assert(expert == 3);
    }
}

pub proof fn each_source_range_lies_inside_its_expert_tile_v1(
    offsets: Seq<nat>,
    expert: nat,
)
    requires valid_offsets_v1(offsets), expert < experts_v1(),
    ensures
        source_range_start_v1(expert) == expert_tile_start_v1(expert),
        source_range_end_v1(offsets, expert) <= expert_tile_end_v1(expert),
{
    every_expert_count_is_capacity_bounded_v1(offsets, expert);
    assert(expert_count_v1(offsets, expert) * output_width_v1()
        <= expert_tile_elements_v1()) by (nonlinear_arith)
        requires expert_count_v1(offsets, expert) <= 4;
}

pub proof fn each_source_coordinate_lies_inside_its_expert_tile_v1(
    offsets: Seq<nat>,
    expert: nat,
    row: nat,
    column: nat,
)
    requires
        valid_offsets_v1(offsets),
        expert < experts_v1(),
        row < expert_count_v1(offsets, expert),
        column < output_width_v1(),
    ensures
        expert_tile_start_v1(expert) <= source_index_v1(offsets, expert, row, column),
        source_index_v1(offsets, expert, row, column) < expert_tile_end_v1(expert),
{
    each_source_range_lies_inside_its_expert_tile_v1(offsets, expert);
    assert(row * output_width_v1() + column
        < expert_count_v1(offsets, expert) * output_width_v1()) by (nonlinear_arith)
        requires
            row < expert_count_v1(offsets, expert),
            column < output_width_v1();
}

pub proof fn each_destination_range_lies_inside_compact_tile_v1(
    offsets: Seq<nat>,
    expert: nat,
)
    requires valid_offsets_v1(offsets), expert < experts_v1(),
    ensures
        destination_range_start_v1(offsets, expert)
            <= destination_range_end_v1(offsets, expert),
        destination_range_end_v1(offsets, expert) <= compact_tile_elements_v1(),
{
    valid_offsets_are_route_bounded_v1(offsets);
    if expert == 0 {
    } else if expert == 1 {
    } else if expert == 2 {
    } else {
        assert(expert == 3);
    }
}

pub proof fn each_compact_destination_coordinate_is_bounded_v1(
    offsets: Seq<nat>,
    expert: nat,
    row: nat,
    column: nat,
)
    requires
        valid_offsets_v1(offsets),
        expert < experts_v1(),
        row < expert_count_v1(offsets, expert),
        column < output_width_v1(),
    ensures destination_index_v1(offsets, expert, row, column) < compact_tile_elements_v1(),
{
    each_destination_range_lies_inside_compact_tile_v1(offsets, expert);
    if expert == 0 {
    } else if expert == 1 {
    } else if expert == 2 {
    } else {
        assert(expert == 3);
    }
    assert(offsets[expert as int] <= offsets[(expert + 1) as int]);
    assert(offsets[(expert + 1) as int]
        == offsets[expert as int] + expert_count_v1(offsets, expert));
    assert(destination_range_end_v1(offsets, expert)
        == destination_range_start_v1(offsets, expert)
            + expert_count_v1(offsets, expert) * output_width_v1());
    assert(row * output_width_v1() + column
        < expert_count_v1(offsets, expert) * output_width_v1()) by (nonlinear_arith)
        requires
            row < expert_count_v1(offsets, expert),
            column < output_width_v1();
    assert(destination_index_v1(offsets, expert, row, column)
        < destination_range_end_v1(offsets, expert));
}

pub proof fn earlier_destination_range_ends_before_later_range_v1(
    offsets: Seq<nat>,
    earlier: nat,
    later: nat,
)
    requires
        valid_offsets_v1(offsets),
        earlier < later,
        later < experts_v1(),
    ensures destination_range_end_v1(offsets, earlier)
        <= destination_range_start_v1(offsets, later),
{
    if earlier == 0 {
        if later == 1 {
        } else if later == 2 {
        } else {
            assert(later == 3);
        }
    } else if earlier == 1 {
        if later == 2 {
        } else {
            assert(later == 3);
        }
    } else {
        assert(earlier == 2 && later == 3);
    }
}

pub proof fn nonempty_destination_ranges_are_pairwise_disjoint_and_ordered_v1(
    offsets: Seq<nat>,
    earlier: nat,
    later: nat,
)
    requires
        valid_offsets_v1(offsets),
        earlier < later,
        later < experts_v1(),
        expert_count_v1(offsets, earlier) > 0,
        expert_count_v1(offsets, later) > 0,
    ensures
        destination_range_start_v1(offsets, earlier)
            < destination_range_end_v1(offsets, earlier),
        destination_range_end_v1(offsets, earlier)
            <= destination_range_start_v1(offsets, later),
        destination_range_start_v1(offsets, later)
            < destination_range_end_v1(offsets, later),
        forall |index: nat| !(in_destination_range_v1(offsets, earlier, index)
            && in_destination_range_v1(offsets, later, index)),
{
    earlier_destination_range_ends_before_later_range_v1(offsets, earlier, later);
}

pub proof fn every_destination_member_is_in_accepted_prefix_v1(
    offsets: Seq<nat>,
    index: nat,
)
    requires valid_offsets_v1(offsets), in_destination_union_v1(offsets, index),
    ensures index < accepted_prefix_end_v1(offsets),
{
}

pub proof fn every_accepted_prefix_member_has_a_destination_range_v1(
    offsets: Seq<nat>,
    index: nat,
)
    requires valid_offsets_v1(offsets), index < accepted_prefix_end_v1(offsets),
    ensures in_destination_union_v1(offsets, index),
{
    if index < offsets[1] * output_width_v1() {
        assert(in_destination_range_v1(offsets, 0, index));
    } else if index < offsets[2] * output_width_v1() {
        assert(in_destination_range_v1(offsets, 1, index));
    } else if index < offsets[3] * output_width_v1() {
        assert(in_destination_range_v1(offsets, 2, index));
    } else {
        assert(in_destination_range_v1(offsets, 3, index));
    }
}

pub proof fn destination_union_is_exactly_the_accepted_prefix_v1(offsets: Seq<nat>)
    requires valid_offsets_v1(offsets),
    ensures forall |index: nat| in_destination_union_v1(offsets, index)
        <==> index < accepted_prefix_end_v1(offsets),
{
    assert forall |index: nat| in_destination_union_v1(offsets, index)
        <==> index < accepted_prefix_end_v1(offsets) by {
        if in_destination_union_v1(offsets, index) {
            every_destination_member_is_in_accepted_prefix_v1(offsets, index);
        }
        if index < accepted_prefix_end_v1(offsets) {
            every_accepted_prefix_member_has_a_destination_range_v1(offsets, index);
        }
    }
}

pub proof fn accepted_prefix_fits_compact_tile_v1(offsets: Seq<nat>)
    requires valid_offsets_v1(offsets),
    ensures accepted_prefix_end_v1(offsets) <= compact_tile_elements_v1(),
{
    valid_offsets_are_route_bounded_v1(offsets);
}

pub proof fn zero_fill_preserves_every_accepted_prefix_value_v1(
    offsets: Seq<nat>,
    accepted_values: Seq<int>,
    index: nat,
)
    requires
        valid_offsets_v1(offsets),
        accepted_values.len() == accepted_prefix_end_v1(offsets),
        index < accepted_prefix_end_v1(offsets),
    ensures zero_filled_value_v1(offsets, accepted_values, index)
        == accepted_values[index as int],
{
    accepted_prefix_fits_compact_tile_v1(offsets);
}

pub proof fn zero_fill_defines_every_unused_tail_value_v1(
    offsets: Seq<nat>,
    accepted_values: Seq<int>,
    index: nat,
)
    requires
        valid_offsets_v1(offsets),
        accepted_values.len() == accepted_prefix_end_v1(offsets),
        accepted_prefix_end_v1(offsets) <= index,
        index < compact_tile_elements_v1(),
    ensures zero_filled_value_v1(offsets, accepted_values, index) == 0,
{
}

pub open spec fn authenticated_proof_receipt_claimed_v1() -> bool { false }
pub open spec fn hsa_copy_claimed_v1() -> bool { false }
pub open spec fn machine_address_refinement_claimed_v1() -> bool { false }
pub open spec fn runtime_execution_claimed_v1() -> bool { false }
pub open spec fn gpu_execution_claimed_v1() -> bool { false }
pub open spec fn generalized_profile_claimed_v1() -> bool { false }

pub proof fn compact_plan_assurance_boundary_is_inert_v1()
    ensures
        !authenticated_proof_receipt_claimed_v1(),
        !hsa_copy_claimed_v1(),
        !machine_address_refinement_claimed_v1(),
        !runtime_execution_claimed_v1(),
        !gpu_execution_claimed_v1(),
        !generalized_profile_claimed_v1(),
{
}

} // verus!
