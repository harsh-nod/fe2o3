// Expected-negative R56 mutation: successful release omits one request.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_release_v1(requests: nat) -> nat {
    if requests > 0 { (requests - 1) as nat } else { 0 }
}
pub proof fn mutated_exact_release_preserves_roster_v1(requests: nat)
    requires requests > 0,
    ensures mutated_release_v1(requests) == requests,
{}
}
fn main() {}
