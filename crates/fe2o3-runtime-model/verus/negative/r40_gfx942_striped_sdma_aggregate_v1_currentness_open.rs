// Expected-negative R40 mutation: observation begins while currentness remains open.
use vstd::prelude::*;
verus! {
pub enum CurrentnessPhaseV1 { Open, Closed }
pub open spec fn mutated_currentness_phase_v1() -> CurrentnessPhaseV1 {
    CurrentnessPhaseV1::Open
}
pub proof fn mutated_observation_requires_closed_currentness_v1()
    ensures mutated_currentness_phase_v1() == CurrentnessPhaseV1::Closed,
{}
}
