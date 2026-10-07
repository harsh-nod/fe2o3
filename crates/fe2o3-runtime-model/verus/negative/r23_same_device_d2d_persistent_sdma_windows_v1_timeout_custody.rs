use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_lease_count_v1() -> nat { 0 }
pub proof fn mutated_d2d_timeout_retains_both_leases_v1()
    ensures mutated_timeout_lease_count_v1() == 2, {}
}
