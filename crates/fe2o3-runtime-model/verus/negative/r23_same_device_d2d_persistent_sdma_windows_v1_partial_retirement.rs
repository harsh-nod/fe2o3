use vstd::prelude::*;
verus! {
pub open spec fn mutated_partial_retired_windows_v1() -> nat { 1 }
pub proof fn mutated_d2d_partial_aggregate_may_retire_prefix_v1()
    ensures mutated_partial_retired_windows_v1() == 0, {}
}
