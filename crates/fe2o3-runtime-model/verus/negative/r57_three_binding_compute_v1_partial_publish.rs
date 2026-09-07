// Expected-negative R57 mutation: a partial owner roster publishes.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_partial_publish_is_atomic_v1() -> bool { true }
pub proof fn mutated_partial_publish_is_rejected_v1()
    ensures !mutated_partial_publish_is_atomic_v1(),
{}
}
fn main() {}
