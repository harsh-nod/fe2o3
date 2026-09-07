// Expected-negative R45 mutation: rollback accepts a stale target event.
use vstd::prelude::*;
verus! {
pub open spec fn bundled_event_v1() -> nat { 37 }
pub open spec fn current_event_v1() -> nat { 38 }
pub proof fn mutated_stale_target_is_current_v1()
    ensures bundled_event_v1() == current_event_v1(),
{}
}
