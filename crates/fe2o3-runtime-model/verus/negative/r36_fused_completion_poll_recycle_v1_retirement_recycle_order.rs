// Expected-negative R36 mutation: dispatch retirement precedes signal reset.
use vstd::prelude::*;

verus! {
pub struct EventsV1 {
    pub reset: nat,
    pub dispatch_recycle: nat,
    pub attachment_recycle: nat,
}

pub open spec fn mutated_recycle_events_v1() -> EventsV1 {
    EventsV1 { reset: 5, dispatch_recycle: 4, attachment_recycle: 8 }
}

pub proof fn mutated_retirement_follows_reset_and_precedes_attachment_v1()
    ensures
        mutated_recycle_events_v1().reset < mutated_recycle_events_v1().dispatch_recycle,
        mutated_recycle_events_v1().dispatch_recycle
            < mutated_recycle_events_v1().attachment_recycle,
{}
}
