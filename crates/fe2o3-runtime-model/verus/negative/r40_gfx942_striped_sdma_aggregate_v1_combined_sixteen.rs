// Expected-negative R40 mutation: combined mode consumes all sixteen queues as striped queues.
use vstd::prelude::*;
verus! {
// Mutation: combined admission uses the standalone upper bound of sixteen
// instead of reserving one directional queue per engine.
pub open spec fn mutated_combined_admitted_v1(queue_count: nat) -> bool {
    queue_count >= 2 && queue_count <= 16 && queue_count % 2 == 0
}
pub proof fn mutated_combined_sixteen_is_rejected_v1()
    ensures !mutated_combined_admitted_v1(16),
{}
}
