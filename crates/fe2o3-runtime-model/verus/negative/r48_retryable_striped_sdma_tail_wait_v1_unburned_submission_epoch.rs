// Expected-negative R48 mutation: an epoch beyond the issuer high-water is accepted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_epoch_admission_v1(epoch: nat, _high_water: nat) -> bool {
    epoch > 0
}
pub proof fn unburned_epoch_is_rejected_v1()
    ensures !mutated_epoch_admission_v1(4, 3),
{}
}
