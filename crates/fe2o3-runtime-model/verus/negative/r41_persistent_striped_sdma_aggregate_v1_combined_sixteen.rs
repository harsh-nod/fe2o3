// Expected-negative R41 mutation: persistent combined mode admits sixteen striped queues.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_combined_sixteen_admitted_v1() -> bool { true }
pub proof fn mutated_combined_sixteen_is_rejected_v1()
    ensures !mutated_combined_sixteen_admitted_v1(),
{}
}
