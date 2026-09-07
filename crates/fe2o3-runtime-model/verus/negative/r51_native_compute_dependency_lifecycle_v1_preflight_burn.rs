// Expected-negative R51 mutation: pure preflight rejection burns an epoch.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_rejected_epoch_v1(epoch: nat) -> nat { epoch + 1 }
pub proof fn mutated_preflight_rejection_is_inert_v1(epoch: nat)
    ensures mutated_rejected_epoch_v1(epoch) == epoch,
{}
}
