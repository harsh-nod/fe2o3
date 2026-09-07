// Expected-negative R41 mutation: a completion ticket substitutes queue occurrence identity.
use vstd::prelude::*;
verus! {
pub open spec fn admitted_queue_generation_v1() -> nat { 8 }
pub open spec fn mutated_ticket_queue_generation_v1() -> nat { 9 }
pub proof fn mutated_ticket_occurrence_is_exact_v1()
    ensures mutated_ticket_queue_generation_v1() == admitted_queue_generation_v1(),
{}
}
