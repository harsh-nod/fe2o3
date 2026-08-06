use vstd::prelude::*;

verus! {

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum WorkgroupPhase {
    Initializing,
    ReadableAfterBarrier,
}

pub open spec fn shared_read_is_legal(phase: WorkgroupPhase, initialized: bool) -> bool {
    phase == WorkgroupPhase::ReadableAfterBarrier && initialized
}

/// Expected failure marker: mutated_read_before_barrier_is_legal.
pub proof fn mutated_read_before_barrier_is_legal()
    ensures
        shared_read_is_legal(WorkgroupPhase::Initializing, true),
{
}

} // verus!
