// Expected-negative R48 mutation: the second receipt mints from the first old epoch.
use vstd::prelude::*;
verus! {
pub open spec fn first_retry_epoch_v1(epoch: nat) -> nat { epoch + 1 }
pub open spec fn mutated_second_retry_epoch_v1(first_old_epoch: nat) -> nat {
    first_old_epoch + 1
}
pub proof fn repeated_retry_epochs_are_strictly_monotonic_v1(epoch: nat)
    ensures first_retry_epoch_v1(epoch) < mutated_second_retry_epoch_v1(epoch),
{}
}
