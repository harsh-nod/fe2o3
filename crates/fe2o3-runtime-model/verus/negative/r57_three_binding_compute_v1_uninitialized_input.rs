// Expected-negative R57 mutation: read role/effect admission omits initialization.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_read_admitted_v1(
    role_is_read: bool, effect_is_read_only: bool, initialized: bool,
) -> bool {
    role_is_read && effect_is_read_only
}
pub proof fn mutated_uninitialized_input_is_rejected_v1(initialized: bool)
    requires !initialized, mutated_read_admitted_v1(true, true, initialized),
    ensures initialized,
{}
}
fn main() {}
