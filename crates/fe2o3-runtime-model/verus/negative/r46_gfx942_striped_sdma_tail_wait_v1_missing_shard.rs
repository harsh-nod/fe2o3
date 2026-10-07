// Expected-negative R46 mutation: an incomplete active-shard roster is admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_shard_count_is_exact_v1(observed: nat, expected: nat) -> bool {
    observed <= expected
}
pub proof fn mutated_missing_shard_is_rejected_v1()
    ensures !mutated_shard_count_is_exact_v1(3, 4),
{}
}
