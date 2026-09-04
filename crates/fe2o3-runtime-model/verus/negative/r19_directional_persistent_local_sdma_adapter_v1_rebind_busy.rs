use vstd::prelude::*;
verus! {
pub open spec fn mutated_can_rebind_v1(idle: bool, frontier_pending: bool) -> bool { true }
pub proof fn mutated_active_or_frontier_state_blocks_rebind_v1()
    ensures !mutated_can_rebind_v1(false, true),
{}
}
