use vstd::prelude::*;

verus! {

pub open spec fn wave64_lanes_v1() -> nat { 64 }
pub open spec fn element_bytes_v1() -> nat { 4 }

pub open spec fn power_of_two_v1(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 {
        1
    } else {
        2 * power_of_two_v1((exponent - 1) as nat)
    }
}

/// Numeric value of the first `end` mask bits in physical-lane order.
pub open spec fn mask_prefix_value_v1(active: Seq<bool>, end: nat) -> nat
    recommends end <= active.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        let lane = (end - 1) as nat;
        mask_prefix_value_v1(active, lane)
            + if active[lane as int] { power_of_two_v1(lane) } else { 0 }
    }
}

/// Binds the logical lane sequence to one concrete 64-bit mask value.
pub open spec fn explicit_wave64_mask_v1(active: Seq<bool>, mask_bits: u64) -> bool {
    active.len() == wave64_lanes_v1()
        && mask_bits as nat == mask_prefix_value_v1(active, wave64_lanes_v1())
}

pub open spec fn fixed_wave64_input_v1(input: Seq<int>) -> bool {
    input.len() == wave64_lanes_v1()
}

pub open spec fn masked_contribution_v1(
    input: Seq<int>,
    active: Seq<bool>,
    lane: nat,
) -> int
    recommends lane < input.len(), lane < active.len(),
{
    if active[lane as int] { input[lane as int] } else { 0 }
}

/// Exact increasing-physical-lane prefix over active contributions.
pub open spec fn masked_prefix_sum_v1(
    input: Seq<int>,
    active: Seq<bool>,
    end: nat,
) -> int
    recommends end <= input.len(), end <= active.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        let lane = (end - 1) as nat;
        masked_prefix_sum_v1(input, active, lane)
            + masked_contribution_v1(input, active, lane)
    }
}

pub open spec fn reduction_value_v1(input: Seq<int>, active: Seq<bool>) -> int {
    masked_prefix_sum_v1(input, active, wave64_lanes_v1())
}

pub open spec fn reduction_output_v1(
    input: Seq<int>,
    active: Seq<bool>,
    lane: nat,
) -> int
    recommends lane < active.len(),
{
    if active[lane as int] { reduction_value_v1(input, active) } else { 0 }
}

pub open spec fn inclusive_output_v1(
    input: Seq<int>,
    active: Seq<bool>,
    lane: nat,
) -> int
    recommends lane < input.len(), lane < active.len(),
{
    if active[lane as int] {
        masked_prefix_sum_v1(input, active, lane + 1)
    } else {
        0
    }
}

pub open spec fn exclusive_output_v1(
    input: Seq<int>,
    active: Seq<bool>,
    lane: nat,
) -> int
    recommends lane < input.len(), lane < active.len(),
{
    if active[lane as int] {
        masked_prefix_sum_v1(input, active, lane)
    } else {
        0
    }
}

pub open spec fn lane_output_index_v1(lane: nat) -> nat { lane }

pub proof fn explicit_mask_has_exact_wave64_width_v1(active: Seq<bool>, mask_bits: u64)
    requires explicit_wave64_mask_v1(active, mask_bits),
    ensures active.len() == 64,
{
}

pub proof fn active_lane_indices_are_in_bounds_v1(
    input: Seq<int>,
    active: Seq<bool>,
    mask_bits: u64,
    lane: nat,
)
    requires
        fixed_wave64_input_v1(input),
        explicit_wave64_mask_v1(active, mask_bits),
        lane < wave64_lanes_v1(),
    ensures
        lane < input.len(),
        lane < active.len(),
        lane_output_index_v1(lane) < wave64_lanes_v1(),
{
}

pub proof fn inactive_lane_is_excluded_and_publishes_zero_v1(
    input: Seq<int>,
    active: Seq<bool>,
    mask_bits: u64,
    lane: nat,
)
    requires
        fixed_wave64_input_v1(input),
        explicit_wave64_mask_v1(active, mask_bits),
        lane < wave64_lanes_v1(),
        !active[lane as int],
    ensures
        masked_contribution_v1(input, active, lane) == 0,
        reduction_output_v1(input, active, lane) == 0,
        inclusive_output_v1(input, active, lane) == 0,
        exclusive_output_v1(input, active, lane) == 0,
{
}

pub proof fn distinct_lanes_have_injective_output_ownership_v1(left: nat, right: nat)
    requires
        left < wave64_lanes_v1(),
        right < wave64_lanes_v1(),
        left != right,
    ensures lane_output_index_v1(left) != lane_output_index_v1(right),
{
}

pub proof fn masked_reduction_step_recurrence_v1(
    input: Seq<int>,
    active: Seq<bool>,
    mask_bits: u64,
    processed: nat,
)
    requires
        fixed_wave64_input_v1(input),
        explicit_wave64_mask_v1(active, mask_bits),
        processed < wave64_lanes_v1(),
    ensures masked_prefix_sum_v1(input, active, processed + 1)
        == masked_prefix_sum_v1(input, active, processed)
            + masked_contribution_v1(input, active, processed),
{
}

pub proof fn reduction_is_full_masked_prefix_v1(
    input: Seq<int>,
    active: Seq<bool>,
    mask_bits: u64,
)
    requires
        fixed_wave64_input_v1(input),
        explicit_wave64_mask_v1(active, mask_bits),
    ensures reduction_value_v1(input, active)
        == masked_prefix_sum_v1(input, active, 64),
{
}

pub proof fn active_lane_scan_recurrence_v1(
    input: Seq<int>,
    active: Seq<bool>,
    mask_bits: u64,
    lane: nat,
)
    requires
        fixed_wave64_input_v1(input),
        explicit_wave64_mask_v1(active, mask_bits),
        lane < wave64_lanes_v1(),
        active[lane as int],
    ensures
        exclusive_output_v1(input, active, lane)
            == masked_prefix_sum_v1(input, active, lane),
        inclusive_output_v1(input, active, lane)
            == exclusive_output_v1(input, active, lane) + input[lane as int],
{
    assert(masked_contribution_v1(input, active, lane) == input[lane as int]);
    assert(masked_prefix_sum_v1(input, active, lane + 1)
        == masked_prefix_sum_v1(input, active, lane)
            + masked_contribution_v1(input, active, lane));
}

pub proof fn empty_mask_has_zero_reduction_and_scans_v1(input: Seq<int>, active: Seq<bool>)
    requires
        fixed_wave64_input_v1(input),
        active.len() == wave64_lanes_v1(),
        forall |lane: int| 0 <= lane < wave64_lanes_v1() ==> !active[lane],
    ensures
        reduction_value_v1(input, active) == 0,
        forall |lane: int| 0 <= lane < wave64_lanes_v1() ==>
            #[trigger] reduction_output_v1(input, active, lane as nat) == 0
                && inclusive_output_v1(input, active, lane as nat) == 0
                && exclusive_output_v1(input, active, lane as nat) == 0,
{
    empty_mask_prefix_is_zero_v1(input, active, wave64_lanes_v1());
}

proof fn empty_mask_prefix_is_zero_v1(input: Seq<int>, active: Seq<bool>, end: nat)
    requires
        fixed_wave64_input_v1(input),
        active.len() == wave64_lanes_v1(),
        end <= wave64_lanes_v1(),
        forall |lane: int| 0 <= lane < wave64_lanes_v1() ==> !active[lane],
    ensures masked_prefix_sum_v1(input, active, end) == 0,
    decreases end,
{
    if end > 0 {
        let lane = (end - 1) as nat;
        empty_mask_prefix_is_zero_v1(input, active, lane);
        assert(!active[lane as int]);
        assert(masked_contribution_v1(input, active, lane) == 0);
    }
}

}
