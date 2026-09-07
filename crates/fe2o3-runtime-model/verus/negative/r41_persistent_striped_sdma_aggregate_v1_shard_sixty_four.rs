// Expected-negative R41 mutation: one striped shard admits sixty-four requests.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_shard_load_v1() -> nat { 64 }
pub proof fn mutated_each_shard_is_bounded_v1()
    ensures mutated_shard_load_v1() <= 63,
{}
}
