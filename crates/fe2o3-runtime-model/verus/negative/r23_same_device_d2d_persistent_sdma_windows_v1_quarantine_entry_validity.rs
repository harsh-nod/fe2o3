use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum PhaseV1 { DevicePairReady, Quarantined }
#[derive(PartialEq, Eq)]
pub enum CustodyV1 { DevicePair, QuarantinedPair }
pub struct StateV1 {
    pub phase: PhaseV1,
    pub custody: CustodyV1,
    pub target_retained: bool,
    pub current: bool,
    pub source_authority_count: nat,
    pub destination_authority_count: nat,
}
pub open spec fn valid_state_v1(state: StateV1) -> bool {
    state.source_authority_count == 1
        && state.destination_authority_count == 1
        && match state.phase {
            PhaseV1::DevicePairReady => state.custody == CustodyV1::DevicePair
                && !state.target_retained && state.current,
            PhaseV1::Quarantined => state.custody == CustodyV1::QuarantinedPair
                && state.target_retained && !state.current,
        }
}
pub open spec fn mutated_quarantine_v1(state: StateV1) -> StateV1 {
    StateV1 { phase: PhaseV1::Quarantined, custody: CustodyV1::QuarantinedPair,
        current: false, ..state }
}
pub open spec fn sample_device_pair_ready_v1() -> StateV1 {
    StateV1 { phase: PhaseV1::DevicePairReady, custody: CustodyV1::DevicePair,
        target_retained: false, current: true, source_authority_count: 1,
        destination_authority_count: 1 }
}
pub proof fn mutated_d2d_quarantine_entry_preserves_validity_v1()
    ensures valid_state_v1(mutated_quarantine_v1(sample_device_pair_ready_v1())), {}
}
