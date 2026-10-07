// Expected-negative R56 mutation: the aggregate admits 127 requests.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_request_capacity_v1(count: nat) -> bool { count <= 127 }
pub proof fn mutated_request_127_is_rejected_v1()
    ensures !mutated_request_capacity_v1(127),
{}
}
fn main() {}
