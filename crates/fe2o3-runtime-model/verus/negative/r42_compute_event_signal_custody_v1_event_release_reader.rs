// Expected-negative R42 mutation: event release also releases a reader pin.
use vstd::prelude::*;
verus! {
pub open spec fn reader_pins_before_v1() -> nat { 1 }
pub open spec fn mutated_reader_pins_after_event_release_v1() -> nat { 0 }
pub proof fn mutated_event_release_preserves_reader_pins_v1()
    ensures mutated_reader_pins_after_event_release_v1() == reader_pins_before_v1(),
{}
}
