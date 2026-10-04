use vstd::prelude::*;

verus! {

pub open spec fn mutated_route_current_without_reset_fence_v1(reset_current: bool) -> bool {
    !reset_current
}

pub proof fn mutated_reset_fence_is_required_for_route_v1()
    ensures !mutated_route_current_without_reset_fence_v1(false),
{
}

} // verus!
