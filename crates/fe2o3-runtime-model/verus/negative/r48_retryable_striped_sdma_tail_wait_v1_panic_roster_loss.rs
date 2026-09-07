// Expected-negative R48 mutation: panic guard drops one request from custody.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_panic_roster_len_v1(requests: nat) -> nat { (requests - 1) as nat }
pub proof fn panic_guard_preserves_whole_roster_v1(requests: nat)
    requires requests > 0,
    ensures mutated_panic_roster_len_v1(requests) == requests,
{}
}
