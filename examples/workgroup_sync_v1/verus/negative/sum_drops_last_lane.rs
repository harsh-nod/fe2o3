use vstd::prelude::*;

#[path = "../workgroup_sync_v1.rs"]
mod model;

verus! {

pub open spec fn mutated_reduction_sum_v1(values: Seq<int>) -> int
    recommends values.len() == model::lane_count_v1(),
{
    model::integer_prefix_sum_v1(values, 63)
}

pub proof fn mutated_reduction_still_equals_exact_sum_v1(values: Seq<int>)
    requires values.len() == model::lane_count_v1(),
    ensures
        mutated_reduction_sum_v1(values)
            == model::integer_prefix_sum_v1(values, model::lane_count_v1()),
{
}

} // verus!
