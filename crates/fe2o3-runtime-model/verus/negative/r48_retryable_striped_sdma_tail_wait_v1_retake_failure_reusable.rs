// Expected-negative R48 mutation: post-retirement retake failure becomes retryable.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { CompletedOpaque, Retryable }
pub open spec fn mutated_retake_failure_v1() -> PhaseV1 { PhaseV1::Retryable }
pub proof fn retake_failure_is_completed_opaque_v1()
    ensures mutated_retake_failure_v1() == PhaseV1::CompletedOpaque,
{}
}
