// Expected-negative R57 mutation: two bindings are admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cardinality_two_admitted_v1() -> bool { true }
pub proof fn mutated_cardinality_two_is_rejected_v1()
    ensures !mutated_cardinality_two_admitted_v1(),
{}
}
fn main() {}
