use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Ready, ProcessTeardown }
pub open spec fn mutated_currentness_loss_v1(_phase: PhaseV1) -> PhaseV1 { PhaseV1::Ready }
pub proof fn mutated_currentness_loss_enters_teardown_v1()
    ensures mutated_currentness_loss_v1(PhaseV1::Ready) == PhaseV1::ProcessTeardown, {}
}
