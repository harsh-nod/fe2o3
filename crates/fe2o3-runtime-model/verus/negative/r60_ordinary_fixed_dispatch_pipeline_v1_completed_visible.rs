// Expected-negative R60 mutation: completion exposes status before host commit.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Completed, HostCommitted }
pub open spec fn mutated_visible_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Completed || phase == PhaseV1::HostCommitted
}
pub proof fn mutated_completed_status_is_hidden_v1()
    ensures !mutated_visible_v1(PhaseV1::Completed),
{}
}
