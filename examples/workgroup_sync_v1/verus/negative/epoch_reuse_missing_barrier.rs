use vstd::prelude::*;

#[path = "../workgroup_sync_v1.rs"]
mod model;

verus! {

pub open spec fn mutated_next_publish_without_reuse_v1(epoch: nat) -> nat {
    model::reuse_barrier_v1(epoch)
}

pub proof fn mutated_reuse_precedes_next_publish_v1(epoch: nat)
    ensures
        model::reuse_barrier_v1(epoch)
            < mutated_next_publish_without_reuse_v1(epoch),
{
}

} // verus!
