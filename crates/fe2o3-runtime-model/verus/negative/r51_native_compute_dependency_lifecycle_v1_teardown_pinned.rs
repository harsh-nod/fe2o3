// Expected-negative R51 mutation: teardown is allowed with one event pin.
use vstd::prelude::*;
verus! {
// Mutation: teardown checks only active-target count and ignores all pin counts.
pub open spec fn mutated_teardown_allowed_v1(
    active_targets: nat,
    _event_pins: nat,
    _reader_pins: nat,
    _target_pins: nat,
) -> bool {
    active_targets == 0
}
pub proof fn mutated_pinned_teardown_is_rejected_v1()
    ensures !mutated_teardown_allowed_v1(0, 1, 0, 0),
{}
}
