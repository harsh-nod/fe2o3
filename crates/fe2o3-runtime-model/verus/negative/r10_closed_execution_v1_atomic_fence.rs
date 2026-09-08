use vstd::prelude::*;

verus! {

pub enum FenceOrderV1 {
    Relaxed,
    Release,
}

pub open spec fn mutated_pre_atomic_fence_v1() -> FenceOrderV1 {
    FenceOrderV1::Relaxed
}

pub proof fn mutated_release_atomic_requires_pre_fence_v1()
    ensures mutated_pre_atomic_fence_v1() == FenceOrderV1::Release,
{
}

} // verus!
