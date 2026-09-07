// Expected-negative R51 mutation: a different dependent epoch releases pins.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_observed_epoch_v1(expected: nat) -> nat { expected + 1 }
pub proof fn mutated_completion_epoch_is_exact_v1(expected: nat)
    ensures mutated_observed_epoch_v1(expected) == expected,
{}
}
