// Expected-negative R57 mutation: post-dispatch ambiguity restores owners.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_dispatch_ambiguity_is_restorable_v1() -> bool { true }
pub proof fn mutated_dispatch_ambiguity_restore_is_rejected_v1()
    ensures !mutated_dispatch_ambiguity_is_restorable_v1(),
{}
}
fn main() {}
