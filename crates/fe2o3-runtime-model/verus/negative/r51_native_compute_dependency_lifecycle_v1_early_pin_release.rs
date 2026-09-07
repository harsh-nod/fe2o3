// Expected-negative R51 mutation: Pending releases source reader/event pins.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_pending_pins_v1() -> (nat, nat) { (0, 0) }
pub proof fn mutated_pending_retains_both_pin_classes_v1()
    ensures mutated_pending_pins_v1() == (1, 1),
{}
}
