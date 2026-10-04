use vstd::prelude::*;
verus! {
pub struct TicketV1 { pub generation: nat }
pub struct FrontierV1 { pub ticket: TicketV1 }
pub open spec fn mutated_frontier_v1(t: TicketV1) -> FrontierV1 {
    FrontierV1 { ticket: TicketV1 { generation: t.generation + 1 } }
}
pub proof fn mutated_frontier_retains_exact_ticket_v1()
    ensures mutated_frontier_v1(TicketV1 { generation: 9 }).ticket.generation == 9, {}
}
