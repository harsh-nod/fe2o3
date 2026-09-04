use vstd::prelude::*;
verus! {
pub open spec fn mutated_quarantine_authority_count_v1() -> nat { 1 }
pub proof fn mutated_d2d_quarantine_retains_two_authorities_v1()
    ensures mutated_quarantine_authority_count_v1() == 2, {}
}
