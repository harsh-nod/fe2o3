// Expected-negative R44 mutation: a stale loan generation reclaims custody.
use vstd::prelude::*;
verus! {
pub open spec fn live_generation_v1() -> nat { 43 }
pub open spec fn stale_generation_v1() -> nat { 42 }
pub proof fn mutated_stale_reclaim_is_exact_v1()
    ensures stale_generation_v1() == live_generation_v1(),
{}
}
