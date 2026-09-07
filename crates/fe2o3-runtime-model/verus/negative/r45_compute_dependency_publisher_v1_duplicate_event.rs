// Expected-negative R45 mutation: two reader records share one event.
use vstd::prelude::*;
verus! {
pub open spec fn first_event_v1() -> nat { 71 }
pub open spec fn second_event_v1() -> nat { 71 }
pub proof fn mutated_reader_events_are_distinct_v1()
    ensures first_event_v1() != second_event_v1(),
{}
}
