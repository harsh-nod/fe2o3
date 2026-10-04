use vstd::prelude::*;
verus! {
pub struct TicketV1 { pub queue: nat, pub slot: nat, pub generation: nat }
pub open spec fn mutated_completion_matches_v1(expected: TicketV1, observed: TicketV1) -> bool {
    expected.queue == observed.queue && expected.slot == observed.slot
}
pub proof fn mutated_stale_completion_generation_is_rejected_v1()
    ensures !mutated_completion_matches_v1(
        TicketV1 { queue: 1, slot: 2, generation: 3 },
        TicketV1 { queue: 1, slot: 2, generation: 4 },
    ),
{}
}
