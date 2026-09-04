use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_authority_v1(_prior: nat) -> nat { 0 }
pub proof fn mutated_timeout_retains_published_authority_v1()
    ensures mutated_timeout_authority_v1(1) == 1, {}
}
