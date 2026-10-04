use vstd::prelude::*;
verus! {
pub open spec fn mutated_retained_ticket_v1() -> Option<nat> { None }
pub proof fn mutated_retained_publication_keeps_ticket_v1()
    ensures mutated_retained_ticket_v1().is_some(),
{}
}
