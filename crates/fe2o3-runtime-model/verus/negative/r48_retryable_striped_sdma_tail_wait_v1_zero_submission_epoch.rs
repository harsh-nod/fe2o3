// Expected-negative R48 mutation: submission epoch zero is accepted.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_epoch_admission_v1(epoch: nat, high_water: nat) -> bool {
    epoch <= high_water
}
pub proof fn zero_epoch_is_rejected_v1()
    ensures !mutated_epoch_admission_v1(0, 3),
{}
}
