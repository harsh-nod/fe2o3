// Expected-negative R48 mutation: an early tail error is counted as a full round.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_tail_error_work_v1(rounds: nat, shards: nat, _prefix: nat) -> nat {
    (rounds + 1) * shards
}
pub proof fn tail_error_has_rounds_s_plus_prefix_v1()
    ensures mutated_tail_error_work_v1(2, 6, 3) == 2 * 6 + 3,
{}
}
