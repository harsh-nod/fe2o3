use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Prepared, Quarantined }
pub struct StateV1 { pub phase: PhaseV1, pub live_ticket: bool }
pub open spec fn mutated_preparation_currentness_loss_v1() -> StateV1 {
    StateV1 { phase: PhaseV1::Quarantined, live_ticket: true }
}
pub proof fn mutated_preparation_quarantine_has_no_ticket_v1()
    ensures {
        let state = mutated_preparation_currentness_loss_v1();
        &&& state.phase == PhaseV1::Quarantined
        &&& !state.live_ticket
    },
{}
}
