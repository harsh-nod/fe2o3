// Expected-negative R56 mutation: a singleton aggregate is admitted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_request_capacity_v1(count: nat) -> bool { 1 <= count && count <= 126 }
pub proof fn mutated_request_one_is_rejected_v1()
    ensures !mutated_request_capacity_v1(1),
{}
}
fn main() {}
