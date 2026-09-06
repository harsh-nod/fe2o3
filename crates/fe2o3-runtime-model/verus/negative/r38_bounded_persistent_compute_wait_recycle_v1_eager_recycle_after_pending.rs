// Expected-negative R38 mutation: a Pending observation at the deadline is
// allowed to continue eagerly into a later recycle result.
use vstd::prelude::*;

verus! {
#[derive(PartialEq, Eq)] pub enum OutcomeV1 { Pending, Recycled }

pub open spec fn mutated_zero_deadline_pending_v1() -> OutcomeV1 { OutcomeV1::Recycled }

pub proof fn mutated_pending_boundary_stops_before_recycle_v1()
    ensures mutated_zero_deadline_pending_v1() == OutcomeV1::Pending,
{}
}
