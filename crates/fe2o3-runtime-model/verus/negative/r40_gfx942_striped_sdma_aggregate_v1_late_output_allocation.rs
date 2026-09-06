// Expected-negative R40 mutation: completed output is allocated after publication.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_postpublication_allocations_v1() -> nat { 1 }
pub proof fn mutated_completion_uses_no_late_allocation_v1()
    ensures mutated_postpublication_allocations_v1() == 0,
{}
}
