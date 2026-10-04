use vstd::prelude::*;

#[path = "../memory_safety_v2.rs"]
mod memory_safety_v2;
use memory_safety_v2::*;

verus! {

proof fn mutated_out_of_bounds_is_accepted(
    allocation: Allocation,
    access: ByteRange,
)
    requires
        allocation.byte_len < range_end(access),
    ensures
        range_in_bounds(allocation, access),
{
}

} // verus!
