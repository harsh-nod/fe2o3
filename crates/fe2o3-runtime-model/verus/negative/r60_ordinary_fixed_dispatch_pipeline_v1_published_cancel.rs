// Expected-negative R60 mutation: a published entry remains cancellable.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)] pub enum PhaseV1 { Queued, Prepared, Published }
pub open spec fn mutated_cancel_v1(phase: PhaseV1, tail: bool) -> bool {
    tail && (phase == PhaseV1::Queued || phase == PhaseV1::Prepared || phase == PhaseV1::Published)
}
pub proof fn mutated_published_cancel_is_too_late_v1()
    ensures !mutated_cancel_v1(PhaseV1::Published, true),
{}
}
