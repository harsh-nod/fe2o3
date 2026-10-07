// Expected-negative R37 mutation: an identity-change failure retains the
// expected owner rather than the exact changed owner returned by the lower layer.
use vstd::prelude::*;

verus! {
pub struct StateV1 { pub terminal_owner: nat }

pub open spec fn mutated_identity_change_v1(expected: nat, returned: nat) -> StateV1 {
    StateV1 { terminal_owner: expected }
}

pub proof fn mutated_identity_change_retains_returned_owner_v1(expected: nat, returned: nat)
    requires expected > 0, returned > 0, expected != returned,
    ensures mutated_identity_change_v1(expected, returned).terminal_owner == returned,
{}
}
