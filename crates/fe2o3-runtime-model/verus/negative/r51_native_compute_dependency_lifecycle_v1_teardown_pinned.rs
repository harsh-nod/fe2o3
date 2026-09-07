// Expected-negative R51 mutation: teardown is allowed with one event pin.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_teardown_allowed_v1() -> bool { true }
pub proof fn mutated_pinned_teardown_is_rejected_v1()
    ensures !mutated_teardown_allowed_v1(),
{}
}
