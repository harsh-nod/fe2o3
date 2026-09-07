// Expected-negative R44 mutation: the maximum loan generation advances.
use vstd::prelude::*;
verus! {
pub open spec fn maximum_generation_v1() -> nat { 18_446_744_073_709_551_615 }
pub open spec fn mutated_next_generation_v1() -> nat { maximum_generation_v1() + 1 }
pub proof fn mutated_generation_overflow_is_admitted_v1()
    ensures mutated_next_generation_v1() <= maximum_generation_v1(),
{}
}
