// Expected-negative R60 mutation: live completion is treated as settled publication authority.
use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Completed, HostCommitted }

pub open spec fn mutated_completion_authority_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Completed || phase == PhaseV1::HostCommitted
}

pub proof fn mutated_live_completion_authority_is_rejected_v1()
    ensures !mutated_completion_authority_v1(PhaseV1::Completed),
{
}

}
