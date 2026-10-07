// Expected-negative R46 mutation: a noncompleted outcome loses its owner.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_noncompleted_owner_v1(owner: nat) -> Option<nat> { None }
pub proof fn mutated_noncompleted_custody_is_retained_v1(owner: nat)
    requires owner > 0,
    ensures mutated_noncompleted_owner_v1(owner) == Some(owner),
{}
}
