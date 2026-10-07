use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum ComputePhaseV1 { Published, Completed, Restored }

// Mutation: restore ignores the required Completed predecessor.
pub open spec fn mutated_restore_v1(_before: ComputePhaseV1) -> ComputePhaseV1 {
    ComputePhaseV1::Restored
}
pub proof fn mutated_restore_requires_completion_v1()
    ensures mutated_restore_v1(ComputePhaseV1::Published) != ComputePhaseV1::Restored, {}
}
