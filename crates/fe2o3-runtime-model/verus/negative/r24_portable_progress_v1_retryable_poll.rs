use vstd::prelude::*;
verus! {
pub enum PollCustodyV1 { RuntimeRetained, CallerReturned }
pub open spec fn mutated_poll_custody_before_v1() -> PollCustodyV1 {
    PollCustodyV1::RuntimeRetained
}
pub open spec fn mutated_poll_custody_after_v1() -> PollCustodyV1 {
    PollCustodyV1::CallerReturned
}
pub proof fn mutated_retryable_poll_preserves_custody_v1()
    ensures mutated_poll_custody_before_v1() == mutated_poll_custody_after_v1(), {}
}
