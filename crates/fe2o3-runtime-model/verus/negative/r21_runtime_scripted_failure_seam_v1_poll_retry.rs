use vstd::prelude::*;
verus! {
pub open spec fn mutated_poll_retry_ticket_v1(ticket: nat) -> nat { ticket + 1 }
pub proof fn mutated_poll_retry_is_observation_only_v1()
    ensures mutated_poll_retry_ticket_v1(8) == 8, {}
}
