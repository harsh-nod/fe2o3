// Expected-negative R45 mutation: a post-claim failure remains retryable.
use vstd::prelude::*;
verus! {
pub enum ClaimPhaseV1 { Preflight, Claimed }
pub enum FailureDispositionV1 { Retryable, Terminal }
pub open spec fn mutated_claim_phase_v1() -> ClaimPhaseV1 { ClaimPhaseV1::Claimed }
pub open spec fn mutated_failure_disposition_v1() -> FailureDispositionV1 {
    FailureDispositionV1::Retryable
}
pub proof fn mutated_retry_after_claim_is_terminal_v1()
    ensures mutated_claim_phase_v1() == ClaimPhaseV1::Claimed
        ==> mutated_failure_disposition_v1() == FailureDispositionV1::Terminal,
{}
}
