use vstd::prelude::*;

#[path = "../memory_safety_v2.rs"]
mod memory_safety_v2;
use memory_safety_v2::*;

verus! {

proof fn mutated_deallocated_storage_is_live(
    allocation: Allocation,
    epoch: nat,
)
    requires
        allocation.dead_at == Some(epoch),
        allocation.alive_from <= epoch,
        epoch <= allocation.alive_through,
    ensures
        allocation_live_at(allocation, epoch),
{
}

} // verus!
