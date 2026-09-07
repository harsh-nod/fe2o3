// Expected-negative R48 mutation: timeout drops the published submission owner.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_owner_v1(owner: nat) -> Option<nat> { None }
pub proof fn timeout_returns_exact_owner_v1(owner: nat)
    requires owner > 0,
    ensures mutated_timeout_owner_v1(owner) == Some(owner),
{}
}
