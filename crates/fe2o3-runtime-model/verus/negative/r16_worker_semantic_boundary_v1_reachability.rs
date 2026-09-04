use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { AwaitingResponse, Terminal }

#[derive(PartialEq, Eq)]
pub enum CustodyV1 { InFlight, Indeterminate }

pub struct WorkerStateV1 {
    pub phase: PhaseV1,
    pub custody: Option<CustodyV1>,
}

pub open spec fn reachable_terminal_v1(state: WorkerStateV1) -> bool {
    state.phase != PhaseV1::Terminal || state.custody != Some(CustodyV1::InFlight)
}

pub open spec fn mutated_terminal_response_v1() -> WorkerStateV1 {
    WorkerStateV1 {
        phase: PhaseV1::Terminal,
        custody: Some(CustodyV1::InFlight),
    }
}

pub proof fn mutated_terminal_response_preserves_reachability_v1()
    ensures reachable_terminal_v1(mutated_terminal_response_v1()),
{
}

}
