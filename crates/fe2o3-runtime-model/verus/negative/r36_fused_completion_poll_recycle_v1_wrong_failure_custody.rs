// Expected-negative R36 mutation: reset failure substitutes Recycled custody
// for the exact still-Completed authority.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum CustodyV1 { Completed, Recycled }
pub struct StateV1 { pub reset_succeeded: bool, pub custody: CustodyV1 }

pub open spec fn mutated_reset_failure_v1() -> StateV1 {
    StateV1 { reset_succeeded: false, custody: CustodyV1::Recycled }
}

pub proof fn mutated_reset_failure_retains_completed_v1()
    ensures mutated_reset_failure_v1().custody == CustodyV1::Completed,
{}
}
