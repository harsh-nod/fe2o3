// Expected-negative R39 mutation: the scoped profile substitutes the default
// 25us sleep for the exact 50us active-spin floor.
use vstd::prelude::*;

verus! {
pub open spec fn mutated_scoped_floor_ns_v1() -> nat { 25_000 }

pub proof fn mutated_scoped_floor_is_exactly_50000ns_v1()
    ensures mutated_scoped_floor_ns_v1() == 50_000,
{}
}
