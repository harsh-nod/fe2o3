use vstd::prelude::*;

#[path = "../workgroup_sync_v1.rs"]
mod model;

verus! {

pub open spec fn mutated_all_lane_sum_v1(values: Seq<nat>, end: nat) -> nat
    recommends end <= values.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        mutated_all_lane_sum_v1(values, (end - 1) as nat) + values[(end - 1) as int]
    }
}

pub proof fn mutated_atomic_sum_respects_eligibility_v1(
    values: Seq<nat>,
    eligible: Seq<bool>,
)
    requires
        values.len() == model::lane_count_v1(),
        eligible.len() == model::lane_count_v1(),
    ensures
        mutated_all_lane_sum_v1(values, model::lane_count_v1())
            == model::eligible_prefix_sum_v1(values, eligible, model::lane_count_v1()),
{
}

} // verus!
