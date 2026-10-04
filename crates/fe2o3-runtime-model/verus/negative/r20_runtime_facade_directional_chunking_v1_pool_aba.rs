use vstd::prelude::*;
verus! {
pub open spec fn mutated_same_allocation_v1(identity_a: nat, _pool_a: nat,
    identity_b: nat, _pool_b: nat) -> bool { identity_a == identity_b }
pub proof fn mutated_pool_generation_aba_is_rejected_v1()
    ensures !mutated_same_allocation_v1(7, 1, 7, 2), {}
}
