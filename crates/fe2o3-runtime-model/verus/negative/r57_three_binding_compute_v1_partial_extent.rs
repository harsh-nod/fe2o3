// Expected-negative R57 mutation: a partial extent is admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_partial_extent_admitted_v1() -> bool { true }
pub proof fn mutated_partial_extent_is_rejected_v1()
    ensures !mutated_partial_extent_admitted_v1(),
{}
}
fn main() {}
