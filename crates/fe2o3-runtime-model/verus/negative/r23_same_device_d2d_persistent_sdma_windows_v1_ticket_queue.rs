use vstd::prelude::*;
verus! {
pub open spec fn mutated_ticket_queue_v1(expected: nat) -> nat { expected + 1 }
pub proof fn mutated_d2d_ticket_may_bind_foreign_queue_v1()
    ensures mutated_ticket_queue_v1(5) == 5, {}
}
