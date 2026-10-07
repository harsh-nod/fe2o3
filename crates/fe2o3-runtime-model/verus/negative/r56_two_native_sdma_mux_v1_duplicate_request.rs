// Expected-negative R56 mutation: duplicate request tokens are accepted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_distinct_v1(left: nat, right: nat) -> bool { true }
pub proof fn mutated_duplicate_request_is_rejected_v1(token: nat)
    ensures !mutated_distinct_v1(token, token),
{}
}
fn main() {}
