// Expected-negative R45 mutation: a post-claim failure remains retryable.
use vstd::prelude::*;
verus! {
pub open spec fn claim_attempted_v1() -> bool { true }
pub open spec fn retryable_v1() -> bool { true }
pub proof fn mutated_retry_after_claim_is_terminal_v1()
    ensures claim_attempted_v1() ==> !retryable_v1(),
{}
}
