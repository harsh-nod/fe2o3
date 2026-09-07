// Expected-negative R57 mutation: unequal elementwise extents are admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_unequal_extent_is_admitted_v1() -> bool { true }
pub proof fn mutated_unequal_extent_is_rejected_v1()
    ensures !mutated_unequal_extent_is_admitted_v1(),
{}
}
fn main() {}
