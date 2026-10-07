// Expected-negative R60 mutation: WaitForPrior incorrectly requires success.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum StatusV1 { Succeeded, Failed }
pub open spec fn mutated_order_ready_v1(completed: bool, status: StatusV1) -> bool {
    completed && status == StatusV1::Succeeded
}
pub proof fn mutated_ordered_failure_is_completion_ready_v1()
    ensures mutated_order_ready_v1(true, StatusV1::Failed),
{}
}
