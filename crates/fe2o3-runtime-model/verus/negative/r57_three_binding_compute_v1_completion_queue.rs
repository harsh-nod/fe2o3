// Expected-negative R57 mutation: completion substitutes the queue.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_completion_queue_is_authenticated_v1() -> bool { true }
pub proof fn mutated_completion_queue_is_rejected_v1()
    ensures !mutated_completion_queue_is_authenticated_v1(),
{}
}
fn main() {}
