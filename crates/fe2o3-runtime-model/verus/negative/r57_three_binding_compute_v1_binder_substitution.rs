// Expected-negative R57 mutation: fixed-binder authority is substituted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_binder_substitution_preserves_identity_v1() -> bool { true }
pub proof fn mutated_binder_substitution_is_rejected_v1()
    ensures !mutated_binder_substitution_preserves_identity_v1(),
{}
}
fn main() {}
