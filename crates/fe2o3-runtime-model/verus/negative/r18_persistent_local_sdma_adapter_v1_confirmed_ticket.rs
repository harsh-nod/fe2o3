use vstd::prelude::*;
verus! {
pub struct TicketV1 { pub queue: nat, pub slot: nat, pub generation: nat }
pub open spec fn mutated_confirm_v1(expected: TicketV1, observed: TicketV1) -> bool {
    expected.queue == observed.queue
}
pub proof fn mutated_stale_ticket_cannot_confirm_publication_v1()
    ensures !mutated_confirm_v1(
        TicketV1 { queue: 1, slot: 2, generation: 3 },
        TicketV1 { queue: 1, slot: 2, generation: 4 },
    ),
{}
}
