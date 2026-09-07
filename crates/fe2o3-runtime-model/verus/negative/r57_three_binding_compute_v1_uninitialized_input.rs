// Expected-negative R57 mutation: an uninitialized read input is admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_uninitialized_input_admitted_v1() -> bool { true }
pub proof fn mutated_uninitialized_input_is_rejected_v1()
    ensures !mutated_uninitialized_input_admitted_v1(),
{}
}
fn main() {}
