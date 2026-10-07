// Expected-negative R51 mutation: the 129th active target is admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_bound_v1() -> nat { 129 }
pub proof fn mutated_active_bound_is_exact_v1()
    ensures mutated_bound_v1() == 128,
{}
}
