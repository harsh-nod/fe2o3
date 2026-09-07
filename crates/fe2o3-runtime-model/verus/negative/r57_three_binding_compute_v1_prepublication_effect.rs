// Expected-negative R57 mutation: prepublication failure records an effect.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_prepublication_effect_is_restorable_v1() -> bool { true }
pub proof fn mutated_prepublication_effect_is_rejected_v1()
    ensures !mutated_prepublication_effect_is_restorable_v1(),
{}
}
fn main() {}
