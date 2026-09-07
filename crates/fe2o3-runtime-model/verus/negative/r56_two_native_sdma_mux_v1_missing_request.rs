// Expected-negative R56 mutation: a presentation may omit one request ticket.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_roster_v1(requests: nat, tickets: nat) -> bool {
    tickets + 1 == requests
}
pub proof fn mutated_missing_request_is_rejected_v1()
    ensures !mutated_roster_v1(2, 1),
{}
}
fn main() {}
