use vstd::prelude::*;
verus! {
pub open spec fn mutated_opaque_authority_v1(_prior: nat) -> nat { 0 }
pub proof fn mutated_opaque_failure_retains_authority_v1()
    ensures mutated_opaque_authority_v1(1) == 1, {}
}
