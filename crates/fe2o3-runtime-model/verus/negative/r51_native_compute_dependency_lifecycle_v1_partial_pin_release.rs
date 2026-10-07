// Expected-negative R51 mutation: completion releases a reader but not its event.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_completed_pins_v1() -> (nat, nat) { (1, 0) }
pub proof fn mutated_completion_releases_reader_and_event_v1()
    ensures mutated_completed_pins_v1() == (0, 0),
{}
}
