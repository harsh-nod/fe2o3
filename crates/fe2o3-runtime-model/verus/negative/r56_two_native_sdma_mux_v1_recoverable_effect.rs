// Expected-negative R56 mutation: recoverable rejection publishes one native.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_recoverable_native_effect_v1() -> nat { 1 }
pub proof fn mutated_recoverable_is_native_effect_free_v1()
    ensures mutated_recoverable_native_effect_v1() == 0,
{}
}
fn main() {}
