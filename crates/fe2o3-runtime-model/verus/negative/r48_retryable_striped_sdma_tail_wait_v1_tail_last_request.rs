// Expected-negative R48 mutation: normalized shard zero binds its first, not last, request.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_last_request_v1(_requests: nat, _queues: nat, slot: nat) -> nat { slot }
pub proof fn ten_over_eight_tail_zero_is_request_eight_v1()
    ensures mutated_last_request_v1(10, 8, 0) == 8,
{}
}
