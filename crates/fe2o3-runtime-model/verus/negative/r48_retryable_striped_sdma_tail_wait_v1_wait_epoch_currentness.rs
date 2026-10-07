// Expected-negative R48 mutation: currentness ignores the retry wait epoch.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_currentness_v1(owner_epoch: nat, current_epoch: nat) -> bool {
    owner_epoch > 0 && current_epoch > 0
}
pub proof fn stale_wait_epoch_is_rejected_v1()
    ensures !mutated_currentness_v1(2, 1),
{}
}
