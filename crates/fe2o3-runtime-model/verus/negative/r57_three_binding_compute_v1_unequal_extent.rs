// Expected-negative R57 mutation: independently full but unequal extents are admitted.
use vstd::prelude::*;
verus! {
pub struct ExtentsV1 { pub a: nat, pub b: nat, pub c: nat }
pub open spec fn mutated_elementwise_extents_v1(extents: ExtentsV1) -> bool {
    extents.a > 0 && extents.b > 0 && extents.c > 0
}
pub proof fn mutated_unequal_extent_is_rejected_v1(extents: ExtentsV1)
    requires mutated_elementwise_extents_v1(extents), extents.a != extents.c,
    ensures extents.a == extents.b && extents.a == extents.c,
{}
}
fn main() {}
