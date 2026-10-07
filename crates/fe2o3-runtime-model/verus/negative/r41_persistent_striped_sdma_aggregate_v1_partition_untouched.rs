// Expected-negative R41 mutation: publication drops one shard from the custody partition.
use vstd::prelude::*;
verus! {
pub open spec fn queue_count_v1() -> nat { 4 }
pub open spec fn mutated_partition_count_v1() -> nat { 3 }
pub proof fn mutated_partition_is_exact_v1()
    ensures mutated_partition_count_v1() == queue_count_v1(),
{}
}
