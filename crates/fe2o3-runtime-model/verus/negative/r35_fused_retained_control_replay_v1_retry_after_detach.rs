// Expected-negative R35 mutation: detached Storage custody becomes retryable.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Retryable, Terminal }
#[derive(PartialEq, Eq)] pub enum CustodyV1 { RetryableInput, TerminalStorage }
pub struct StateV1 { pub detached: bool, pub outcome: OutcomeV1, pub custody: CustodyV1 }

pub open spec fn mutated_storage_failure_v1() -> StateV1 {
    StateV1 { detached: true, outcome: OutcomeV1::Retryable, custody: CustodyV1::RetryableInput }
}

pub proof fn mutated_retry_after_detach_is_terminal_storage_v1()
    ensures mutated_storage_failure_v1().custody == CustodyV1::TerminalStorage,
{}
}
