// Expected-negative R48 mutation: timeout retry reuses the old wait epoch.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_retry_epoch_v1(epoch: nat) -> nat { epoch }
pub proof fn retry_strictly_advances_wait_epoch_v1(epoch: nat)
    requires epoch > 0,
    ensures mutated_retry_epoch_v1(epoch) > epoch,
{}
}
