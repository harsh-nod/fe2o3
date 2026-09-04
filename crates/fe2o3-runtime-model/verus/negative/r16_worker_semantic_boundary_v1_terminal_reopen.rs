use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { ReadyV5, Terminal }

pub struct WorkerStateV1 {
    pub phase: PhaseV1,
}

pub open spec fn mutated_terminal_response_v1() -> WorkerStateV1 {
    WorkerStateV1 { phase: PhaseV1::ReadyV5 }
}

pub open spec fn mutated_receive_after_terminal_v1(_state: WorkerStateV1) -> WorkerStateV1 {
    WorkerStateV1 { phase: PhaseV1::ReadyV5 }
}

pub proof fn mutated_terminal_response_seals_and_absorbs_v1()
    ensures {
        let sealed = mutated_terminal_response_v1();
        &&& sealed.phase == PhaseV1::Terminal
        &&& mutated_receive_after_terminal_v1(sealed) == sealed
    },
{
}

}
