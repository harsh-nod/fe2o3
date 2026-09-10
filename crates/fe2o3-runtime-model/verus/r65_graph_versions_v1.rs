// Abstract per-segment guards only. Ledger, epoch and backend refinement are external.
use vstd::prelude::*;
verus! {
pub enum PhaseV1 { AvailableAtAdmission, Planned, InFlight, Committed, NotProduced, Failed }
pub open spec fn input_ready_v1(current: Option<nat>, expected: nat, phase: PhaseV1) -> bool {
    current == Some(expected) && (phase == PhaseV1::AvailableAtAdmission || phase == PhaseV1::Committed)
}
pub open spec fn begin_write_v1(phase: PhaseV1, pending: Option<nat>, current: Option<nat>, predecessor: nat) -> bool {
    phase == PhaseV1::Planned && pending == None && current == Some(predecessor)
}
pub open spec fn commit_write_v1(phase: PhaseV1, pending: Option<nat>, output: nat, current: Option<nat>) -> bool {
    phase == PhaseV1::InFlight && pending == Some(output) && current == None
}
pub proof fn input_exact_v1(current: Option<nat>, expected: nat, phase: PhaseV1)
    requires input_ready_v1(current, expected, phase),
    ensures current == Some(expected),
{}
pub proof fn input_available_v1(current: Option<nat>, expected: nat, phase: PhaseV1)
    requires input_ready_v1(current, expected, phase),
    ensures phase == PhaseV1::AvailableAtAdmission || phase == PhaseV1::Committed,
{}
pub proof fn begin_planned_v1(phase: PhaseV1, pending: Option<nat>, current: Option<nat>, predecessor: nat)
    requires begin_write_v1(phase, pending, current, predecessor),
    ensures phase == PhaseV1::Planned,
{}
pub proof fn begin_exclusive_v1(phase: PhaseV1, pending: Option<nat>, current: Option<nat>, predecessor: nat)
    requires begin_write_v1(phase, pending, current, predecessor),
    ensures pending == None,
{}
pub proof fn begin_predecessor_v1(phase: PhaseV1, pending: Option<nat>, current: Option<nat>, predecessor: nat)
    requires begin_write_v1(phase, pending, current, predecessor),
    ensures current == Some(predecessor),
{}
pub proof fn commit_in_flight_v1(phase: PhaseV1, pending: Option<nat>, output: nat, current: Option<nat>)
    requires commit_write_v1(phase, pending, output, current),
    ensures phase == PhaseV1::InFlight,
{}
pub proof fn commit_exact_owner_v1(phase: PhaseV1, pending: Option<nat>, output: nat, current: Option<nat>)
    requires commit_write_v1(phase, pending, output, current),
    ensures pending == Some(output),
{}
pub proof fn commit_invalidated_v1(phase: PhaseV1, pending: Option<nat>, output: nat, current: Option<nat>)
    requires commit_write_v1(phase, pending, output, current),
    ensures current == None,
{}
}
