use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum ScopePhaseV1 { Active, Closed, TerminalAbsorbed }

#[derive(PartialEq, Eq)]
pub enum ControlStateV1 { Ordinary, DataDetached }

#[derive(PartialEq, Eq)]
pub enum AttemptPhaseV1 { Available, Prepared }

#[derive(PartialEq, Eq)]
pub struct CloseStateV1 {
    pub scope: ScopePhaseV1,
    pub control: ControlStateV1,
    pub attempt_phase: AttemptPhaseV1,
    pub attempt_present: bool,
    pub stable_matches: bool,
    pub full_close_audits: nat,
}

// Mutation of the positive close_scope_v1 transition: closeability and stable
// authentication remain in their real order, but the full close audit is lost.
pub open spec fn mutated_close_without_full_audit_v1(state: CloseStateV1) -> CloseStateV1 {
    if state.scope == ScopePhaseV1::TerminalAbsorbed { state }
    else if state.scope != ScopePhaseV1::Active
        || state.control != ControlStateV1::DataDetached
        || state.attempt_phase != AttemptPhaseV1::Available || state.attempt_present { state }
    else if !state.stable_matches {
        CloseStateV1 { scope: ScopePhaseV1::TerminalAbsorbed, ..state }
    } else {
        CloseStateV1 {
            scope: ScopePhaseV1::Closed,
            control: ControlStateV1::Ordinary,
            full_close_audits: state.full_close_audits,
            ..state
        }
    }
}

pub proof fn mutated_close_performs_one_full_audit_v1(state: CloseStateV1)
    requires state.scope == ScopePhaseV1::Active,
        state.control == ControlStateV1::DataDetached,
        state.attempt_phase == AttemptPhaseV1::Available,
        !state.attempt_present, state.stable_matches, state.full_close_audits == 0,
    ensures mutated_close_without_full_audit_v1(state).full_close_audits == 1, {}

}
