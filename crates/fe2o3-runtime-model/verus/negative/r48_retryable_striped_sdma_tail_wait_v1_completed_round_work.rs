// Expected-negative R48 mutation: a completed tail round omits one active shard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_round_work_v1(rounds: nat, shards: nat) -> nat {
    rounds * ((shards - 1) as nat)
}
pub proof fn completed_rounds_have_rounds_times_s_work_v1()
    ensures mutated_round_work_v1(3, 6) == 3 * 6,
{}
}
