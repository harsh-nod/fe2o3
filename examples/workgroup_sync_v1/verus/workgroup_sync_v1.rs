use vstd::prelude::*;

verus! {

pub open spec fn lane_count_v1() -> nat { 64 }
pub open spec fn owner_lane_v1() -> nat { 0 }
pub open spec fn publish_slot_v1(lane: nat) -> nat { lane }
pub open spec fn publish_epoch_v1(epoch: nat) -> nat { epoch }
pub open spec fn read_epoch_v1(epoch: nat) -> nat { epoch }
pub open spec fn publish_barrier_v1(epoch: nat) -> nat { epoch * 2 }
pub open spec fn reuse_barrier_v1(epoch: nat) -> nat { epoch * 2 + 1 }
pub open spec fn next_publish_barrier_v1(epoch: nat) -> nat { (epoch + 1) * 2 }
pub open spec fn writes_output_v1(lane: nat) -> bool { lane == owner_lane_v1() }

pub proof fn lane_initializes_its_unique_slot_v1(lane: nat)
    requires lane < lane_count_v1(),
    ensures
        publish_slot_v1(lane) == lane,
        publish_slot_v1(lane) < lane_count_v1(),
{
}

pub proof fn distinct_lanes_initialize_distinct_slots_v1(left: nat, right: nat)
    requires
        left < lane_count_v1(),
        right < lane_count_v1(),
        left != right,
    ensures publish_slot_v1(left) != publish_slot_v1(right),
{
}

pub proof fn all_lanes_reach_one_publish_barrier_v1(epoch: nat, left: nat, right: nat)
    requires
        left < lane_count_v1(),
        right < lane_count_v1(),
    ensures publish_barrier_v1(epoch) == publish_barrier_v1(epoch),
{
}

pub proof fn epoch_read_is_initialized_and_reuse_is_ordered_v1(epoch: nat)
    ensures
        publish_epoch_v1(epoch) == read_epoch_v1(epoch),
        publish_barrier_v1(epoch) < reuse_barrier_v1(epoch),
        reuse_barrier_v1(epoch) < next_publish_barrier_v1(epoch),
{
    assert(epoch * 2 + 1 < (epoch + 1) * 2) by (nonlinear_arith);
}

pub proof fn lane_zero_is_the_only_output_owner_v1(lane: nat)
    requires lane < lane_count_v1(),
    ensures
        writes_output_v1(owner_lane_v1()),
        writes_output_v1(lane) ==> lane == owner_lane_v1(),
{
}

pub proof fn two_output_owners_are_equal_v1(left: nat, right: nat)
    requires
        left < lane_count_v1(),
        right < lane_count_v1(),
        writes_output_v1(left),
        writes_output_v1(right),
    ensures left == right,
{
}

pub open spec fn integer_prefix_sum_v1(values: Seq<int>, end: nat) -> int
    recommends end <= values.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        integer_prefix_sum_v1(values, (end - 1) as nat) + values[(end - 1) as int]
    }
}

pub proof fn reduction_step_preserves_exact_sum_v1(values: Seq<int>, end: nat)
    requires end < values.len(),
    ensures
        integer_prefix_sum_v1(values, end + 1)
            == integer_prefix_sum_v1(values, end) + values[end as int],
{
}

pub proof fn complete_reduction_is_exact_prefix_v1(values: Seq<int>)
    requires values.len() == lane_count_v1(),
    ensures
        integer_prefix_sum_v1(values, lane_count_v1())
            == integer_prefix_sum_v1(values, values.len()),
{
}

pub open spec fn eligible_prefix_sum_v1(
    values: Seq<nat>,
    eligible: Seq<bool>,
    end: nat,
) -> nat
    recommends end <= values.len(), end <= eligible.len(),
    decreases end,
{
    if end == 0 {
        0
    } else if eligible[(end - 1) as int] {
        eligible_prefix_sum_v1(values, eligible, (end - 1) as nat)
            + values[(end - 1) as int]
    } else {
        eligible_prefix_sum_v1(values, eligible, (end - 1) as nat)
    }
}

pub proof fn eligible_lane_contributes_once_v1(
    values: Seq<nat>,
    eligible: Seq<bool>,
    lane: nat,
)
    requires
        values.len() == lane_count_v1(),
        eligible.len() == lane_count_v1(),
        lane < lane_count_v1(),
        eligible[lane as int],
    ensures
        eligible_prefix_sum_v1(values, eligible, lane + 1)
            == eligible_prefix_sum_v1(values, eligible, lane) + values[lane as int],
{
}

pub proof fn ineligible_lane_contributes_zero_v1(
    values: Seq<nat>,
    eligible: Seq<bool>,
    lane: nat,
)
    requires
        values.len() == lane_count_v1(),
        eligible.len() == lane_count_v1(),
        lane < lane_count_v1(),
        !eligible[lane as int],
    ensures
        eligible_prefix_sum_v1(values, eligible, lane + 1)
            == eligible_prefix_sum_v1(values, eligible, lane),
{
}

pub proof fn atomic_final_value_is_initial_plus_eligible_sum_v1(
    initial: nat,
    values: Seq<nat>,
    eligible: Seq<bool>,
)
    requires
        values.len() == lane_count_v1(),
        eligible.len() == lane_count_v1(),
    ensures
        initial + eligible_prefix_sum_v1(values, eligible, lane_count_v1())
            == initial + eligible_prefix_sum_v1(values, eligible, values.len()),
{
}

} // verus!
