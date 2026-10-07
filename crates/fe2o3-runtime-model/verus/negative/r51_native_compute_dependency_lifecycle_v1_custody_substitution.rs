// Expected-negative R51 mutation: Pending returns substituted public custody.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_pending_custody_v1(before: nat) -> nat { before + 1 }
pub proof fn mutated_pending_public_custody_is_stable_v1(before: nat)
    ensures mutated_pending_custody_v1(before) == before,
{}
}
