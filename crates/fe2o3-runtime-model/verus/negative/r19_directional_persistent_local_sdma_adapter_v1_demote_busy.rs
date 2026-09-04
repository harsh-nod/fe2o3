use vstd::prelude::*;
verus! {
pub open spec fn mutated_can_demote_v1(idle: bool, occupied: nat) -> bool { occupied <= 1 }
pub proof fn mutated_active_or_frontier_state_blocks_demotion_v1()
    ensures !mutated_can_demote_v1(false, 1),
{}
}
