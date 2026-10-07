// Expected-negative R40 mutation: queue slot and generation are substituted in a completion ticket.
use vstd::prelude::*;
verus! {
pub struct TicketV1 { pub slot: nat, pub generation: nat }
pub open spec fn mutated_ticket_v1(ticket: TicketV1) -> TicketV1 {
    TicketV1 { slot: ticket.slot + 1, generation: ticket.generation + 1 }
}
pub proof fn mutated_ticket_is_exact_v1(ticket: TicketV1)
    ensures mutated_ticket_v1(ticket).slot == ticket.slot
        && mutated_ticket_v1(ticket).generation == ticket.generation,
{}
}
