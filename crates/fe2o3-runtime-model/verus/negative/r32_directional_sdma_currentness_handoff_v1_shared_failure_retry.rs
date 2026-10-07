// Expected-negative R32 mutation: shared-check failure is exposed as retryable request custody.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Retryable, Terminal }
#[derive(PartialEq, Eq)] pub enum CustodyV1 { Request, Prepared }
pub struct StateV1 { pub outcome: OutcomeV1, pub custody: CustodyV1, pub published: bool }

pub open spec fn mutated_shared_failure_v1() -> StateV1 {
    StateV1 { outcome: OutcomeV1::Retryable, custody: CustodyV1::Request, published: false }
}

pub proof fn mutated_shared_failure_is_terminal_prepared_v1()
    ensures mutated_shared_failure_v1().outcome == OutcomeV1::Terminal,
        mutated_shared_failure_v1().custody == CustodyV1::Prepared,
        !mutated_shared_failure_v1().published,
{}

}
