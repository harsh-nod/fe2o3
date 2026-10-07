// Expected-negative R46 mutation: every round is counted as a full N scan.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_work_v1(rounds: nat, request_count: nat) -> nat {
    rounds * request_count
}
pub proof fn mutated_false_complexity_count_is_exact_v1()
    ensures mutated_work_v1(3, 8) == 3 * 2 + 8,
{}
}
