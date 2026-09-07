// Expected-negative R45 mutation: a same-queue dependency is cross-queue.
use vstd::prelude::*;
verus! {
pub open spec fn source_queue_v1() -> nat { 31 }
pub open spec fn target_queue_v1() -> nat { 31 }
pub proof fn mutated_same_queue_source_is_cross_queue_v1()
    ensures source_queue_v1() != target_queue_v1(),
{}
}
