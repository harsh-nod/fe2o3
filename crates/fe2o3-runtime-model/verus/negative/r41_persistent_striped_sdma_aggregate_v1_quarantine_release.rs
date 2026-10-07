// Expected-negative R41 mutation: terminal quarantine transitions back to caller ownership.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub enum StateV1 { CallerOwned, Quarantined }
pub open spec fn mutated_next_v1(_state: StateV1) -> StateV1 { StateV1::CallerOwned }
pub proof fn mutated_quarantine_is_monotonic_v1()
    ensures mutated_next_v1(StateV1::Quarantined) == StateV1::Quarantined,
{}
}
