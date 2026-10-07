use vstd::prelude::*;
verus! {
pub open spec fn mutated_retry_lease_count_v1() -> nat { 2 }
pub proof fn mutated_d2d_prepublication_retry_restores_both_owners_v1()
    ensures mutated_retry_lease_count_v1() == 0, {}
}
