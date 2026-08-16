use vstd::prelude::*;
verus! {
pub open spec fn owner_v1(buffer: nat, index: nat) -> nat { buffer * 32 + index }
pub proof fn mutated_identical_writes_have_distinct_owners_v1(buffer: nat, index: nat)
    ensures owner_v1(buffer, index) != owner_v1(buffer, index),
{
}
}
