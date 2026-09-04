use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_ticket_v1(ticket: nat) -> Option<nat> { None }
pub proof fn mutated_timeout_retains_ticket_v1()
    ensures mutated_timeout_ticket_v1(7) == Some(7),
{}
}
