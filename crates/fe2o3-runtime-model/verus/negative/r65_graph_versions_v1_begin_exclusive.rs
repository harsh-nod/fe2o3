use vstd::prelude::*;
verus! {
pub enum PhaseV1 { AvailableAtAdmission, Planned, InFlight, Committed, NotProduced, Failed }
pub open spec fn input_ready_v1(current: Option<nat>, expected: nat, phase: PhaseV1) -> bool {
    current == Some(expected) && (phase == PhaseV1::AvailableAtAdmission || phase == PhaseV1::Committed)
}
pub open spec fn begin_write_v1(phase: PhaseV1, pending: Option<nat>, current: Option<nat>, predecessor: nat) -> bool {
    phase == PhaseV1::Planned && current == Some(predecessor)
}
pub open spec fn commit_write_v1(phase: PhaseV1, pending: Option<nat>, output: nat, current: Option<nat>) -> bool {
    phase == PhaseV1::InFlight && pending == Some(output) && current == None
}
pub proof fn mutated_begin_exclusive_v1(phase: PhaseV1, pending: Option<nat>, current: Option<nat>, predecessor: nat)
    requires begin_write_v1(phase, pending, current, predecessor),
    ensures pending == None,
{}
}
