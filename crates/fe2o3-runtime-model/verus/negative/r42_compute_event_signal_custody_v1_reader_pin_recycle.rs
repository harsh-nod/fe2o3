// Expected-negative R42 mutation: a completed slot recycles with a reader pin.
use vstd::prelude::*;
verus! {
pub open spec fn reader_pins_v1() -> nat { 1 }
pub open spec fn mutated_can_recycle_v1() -> bool { reader_pins_v1() > 0 }
pub proof fn mutated_reader_pin_blocks_recycle_v1()
    ensures !mutated_can_recycle_v1(),
{}
}
