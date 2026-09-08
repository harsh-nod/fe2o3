// Expected-negative R60 mutation: explicit dependency accepts failure.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum StatusV1 { Succeeded, Failed }
pub open spec fn mutated_explicit_ready_v1(completed: bool, _status: StatusV1) -> bool {
    completed
}
pub proof fn mutated_explicit_failure_is_blocked_v1()
    ensures !mutated_explicit_ready_v1(true, StatusV1::Failed),
{}
}
