use vstd::prelude::*;

verus! {
pub open spec fn mutated_range_valid_v1(offset: nat, len: nat, bound: nat) -> bool {
    len > 0 && offset + len <= bound + 1
}
pub proof fn mutated_out_of_extent_range_is_rejected_v1()
    ensures !mutated_range_valid_v1(268435455, 2, 268435456),
{}
}
