// Expected-negative R48 mutation: retry clears cumulative observation work.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_retry_cumulative_v1(_before: nat) -> nat { 0 }
pub proof fn retry_preserves_cumulative_work_v1()
    ensures mutated_retry_cumulative_v1(23) == 23,
{}
}
