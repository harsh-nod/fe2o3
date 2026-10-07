// Expected-negative R48 mutation: assignment ignores the rotating first queue.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_queue_for_request_v1(_first: nat, queues: nat, index: nat) -> nat {
    index % queues
}
pub proof fn first_request_uses_rotating_cursor_v1()
    ensures mutated_queue_for_request_v1(5, 8, 0) == 5,
{}
}
