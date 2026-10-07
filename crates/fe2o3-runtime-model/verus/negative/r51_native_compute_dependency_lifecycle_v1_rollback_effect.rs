// Expected-negative R51 mutation: ring-full rollback claims a native effect.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_native_effects_v1(before: nat) -> nat { before + 1 }
pub proof fn mutated_retryable_rollback_has_no_native_effect_v1(before: nat)
    ensures mutated_native_effects_v1(before) == before,
{}
}
