use vstd::prelude::*;
verus! {
pub open spec fn mutated_retirement_retry_authority_v1(_prior: nat) -> nat { 0 }
pub proof fn mutated_retirement_retry_retains_exact_authority_v1()
    ensures mutated_retirement_retry_authority_v1(1) == 1, {}
}
