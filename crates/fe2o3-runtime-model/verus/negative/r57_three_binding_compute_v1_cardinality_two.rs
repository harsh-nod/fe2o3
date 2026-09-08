// Expected-negative R57 mutation: preparation admits two or three bindings.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cardinality_admitted_v1(count: nat) -> bool {
    2 <= count <= 3
}
pub proof fn mutated_cardinality_two_is_rejected_v1()
    ensures !mutated_cardinality_admitted_v1(2),
{}
}
fn main() {}
