// Expected-negative R57 mutation: Timeout is handled before closing currentness.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Published, Quarantined }
#[derive(PartialEq, Eq)]
pub enum ObservationV1 { Timeout, Completed }
pub open spec fn mutated_wait_v1(
    closing_currentness: bool, observation: ObservationV1,
) -> PhaseV1 {
    match observation {
        ObservationV1::Timeout => PhaseV1::Published,
        ObservationV1::Completed =>
            if closing_currentness { PhaseV1::Published } else { PhaseV1::Quarantined },
    }
}
pub proof fn mutated_closing_currentness_restore_is_rejected_v1()
    ensures mutated_wait_v1(false, ObservationV1::Timeout) == PhaseV1::Quarantined,
{}
}
fn main() {}
