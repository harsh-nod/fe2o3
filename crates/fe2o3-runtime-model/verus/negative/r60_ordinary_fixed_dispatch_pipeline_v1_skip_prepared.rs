// Expected-negative R60 mutation: a queued entry publishes without preparation.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Queued, Prepared, Published }
pub open spec fn mutated_publish_v1(_phase: PhaseV1) -> PhaseV1 { PhaseV1::Published }
pub proof fn mutated_queued_cannot_publish_v1()
    ensures mutated_publish_v1(PhaseV1::Queued) == PhaseV1::Queued,
{}
}
