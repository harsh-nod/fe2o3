use vstd::prelude::*;
verus! {
pub enum PhaseV1 { SettledFrontierPending, Settled, Quarantined }
pub open spec fn mutated_can_release_v1(phase: PhaseV1) -> bool {
    phase != PhaseV1::Quarantined
}
pub proof fn mutated_pending_frontier_blocks_release_v1()
    ensures !mutated_can_release_v1(PhaseV1::SettledFrontierPending),
{}
}
