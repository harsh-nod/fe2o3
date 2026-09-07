// Expected-negative R41 mutation: failed identity preflight restores a prefix.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_preflight_restored_count_v1() -> nat { 1 }
pub proof fn mutated_failed_preflight_restores_nothing_v1()
    ensures mutated_preflight_restored_count_v1() == 0,
{}
}
