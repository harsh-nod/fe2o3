// Expected-negative R45 mutation: 51 five-way barriers cover 256 sources.
use vstd::prelude::*;
verus! {
pub open spec fn dependency_count_v1() -> nat { 256 }
pub open spec fn barrier_count_v1() -> nat { 51 }
pub proof fn mutated_barrier_packing_covers_roster_v1()
    ensures dependency_count_v1() <= barrier_count_v1() * 5,
{}
}
