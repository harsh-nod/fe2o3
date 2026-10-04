use vstd::prelude::*;
verus! {
pub open spec fn mutated_preparation_quarantine_ticket_v1() -> Option<nat> { Some(1) }
pub proof fn mutated_preparation_quarantine_has_no_ticket_v1()
    ensures mutated_preparation_quarantine_ticket_v1().is_none(),
{}
}
