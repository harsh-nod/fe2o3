use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Ready, Published }
pub open spec fn mutated_poll_v1(_phase: PhaseV1) -> PhaseV1 { PhaseV1::Published }
pub proof fn mutated_poll_never_publishes_continuation_v1()
    ensures mutated_poll_v1(PhaseV1::Ready) == PhaseV1::Ready, {}
}
