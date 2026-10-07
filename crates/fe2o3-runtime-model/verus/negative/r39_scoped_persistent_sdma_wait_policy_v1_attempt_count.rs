// Expected-negative R39 mutation: a live Pending result selects an action
// without first counting the attempt.
use vstd::prelude::*;

verus! {
pub open spec fn mutated_next_attempt_v1(attempts: nat) -> nat { attempts }

pub proof fn mutated_pending_counts_attempt_first_v1(attempts: nat)
    ensures mutated_next_attempt_v1(attempts) == attempts + 1,
{}
}
