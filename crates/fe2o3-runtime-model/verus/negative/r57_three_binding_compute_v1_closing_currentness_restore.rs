// Expected-negative R57 mutation: closing-currentness loss restores owners.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_closing_currentness_is_restorable_v1() -> bool { true }
pub proof fn mutated_closing_currentness_restore_is_rejected_v1()
    ensures !mutated_closing_currentness_is_restorable_v1(),
{}
}
fn main() {}
