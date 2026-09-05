// Expected-negative R36 mutation: Pending continues into signal recycle.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Pending, Recycled }
pub struct StateV1 { pub outcome: OutcomeV1, pub signal_reset: bool }

pub open spec fn mutated_pending_v1() -> StateV1 {
    StateV1 { outcome: OutcomeV1::Pending, signal_reset: true }
}

pub proof fn mutated_pending_short_circuits_before_recycle_v1()
    ensures !mutated_pending_v1().signal_reset,
{}
}
