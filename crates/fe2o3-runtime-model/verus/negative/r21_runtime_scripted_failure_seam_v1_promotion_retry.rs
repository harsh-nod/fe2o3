use vstd::prelude::*;
verus! {
pub open spec fn mutated_promotion_retry_phase_v1(_phase: nat) -> nat { 2 }
pub proof fn mutated_promotion_retry_is_atomic_v1()
    ensures mutated_promotion_retry_phase_v1(1) == 1, {}
}
