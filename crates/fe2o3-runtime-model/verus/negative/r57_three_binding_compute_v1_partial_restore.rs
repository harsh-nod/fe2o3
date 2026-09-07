// Expected-negative R57 mutation: only two owners are restored.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_partial_restore_v1() -> bool { true }
pub proof fn mutated_partial_restore_is_rejected_v1()
    ensures !mutated_partial_restore_v1(),
{}
}
fn main() {}
