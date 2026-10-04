use vstd::prelude::*;

verus! {

pub open spec fn mutated_release_fence_plan_v1() -> bool {
    false
}

pub proof fn mutated_release_atomic_requires_pre_fence_v1()
    ensures mutated_release_fence_plan_v1(),
{
}

} // verus!
