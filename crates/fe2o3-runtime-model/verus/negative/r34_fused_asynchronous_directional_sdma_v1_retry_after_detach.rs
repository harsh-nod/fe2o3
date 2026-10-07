// Expected-negative R34 mutation: detached request custody becomes retryable
// after the enclosing fused loan cannot be retaken.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { RetryableRequest, TerminalPrepared }
pub struct StateV1 {
    pub request_constructed_after_detach: bool,
    pub retake_succeeded: bool,
    pub custody: CustodyV1,
}

pub open spec fn mutated_detached_finish_v1() -> StateV1 {
    StateV1 {
        request_constructed_after_detach: true,
        retake_succeeded: false,
        custody: CustodyV1::RetryableRequest,
    }
}

pub proof fn mutated_retry_after_detach_is_terminal_prepared_v1()
    ensures mutated_detached_finish_v1().request_constructed_after_detach,
        !mutated_detached_finish_v1().retake_succeeded,
        mutated_detached_finish_v1().custody == CustodyV1::TerminalPrepared,
{}
}
