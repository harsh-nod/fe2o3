use vstd::prelude::*;

#[path = "../memory_safety_v2.rs"]
mod memory_safety_v2;
use memory_safety_v2::*;

verus! {

proof fn mutated_stale_generation_is_accepted(
    allocation: Allocation,
    provenance: Provenance,
)
    requires
        allocation.id == provenance.allocation_id,
        allocation.generation != provenance.generation,
    ensures
        provenance_matches(allocation, provenance),
{
}

} // verus!
