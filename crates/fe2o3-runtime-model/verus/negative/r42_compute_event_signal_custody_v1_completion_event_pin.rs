// Expected-negative R42 mutation: completion implicitly releases an event pin.
use vstd::prelude::*;
verus! {
pub open spec fn event_pins_before_v1() -> nat { 1 }
pub open spec fn mutated_event_pins_after_completion_v1() -> nat { 0 }
pub proof fn mutated_completion_preserves_event_pins_v1()
    ensures mutated_event_pins_after_completion_v1() == event_pins_before_v1(),
{}
}
