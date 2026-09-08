// Expected-negative R57 mutation: an in-bounds subrange is treated as full extent.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_extent_admitted_v1(
    offset: nat, byte_len: nat, allocation_len: nat,
) -> bool {
    byte_len > 0 && offset + byte_len <= allocation_len
}
pub proof fn mutated_partial_extent_is_rejected_v1(byte_len: nat, allocation_len: nat)
    requires byte_len > 0, byte_len < allocation_len,
        mutated_extent_admitted_v1(0, byte_len, allocation_len),
    ensures byte_len == allocation_len,
{}
}
fn main() {}
