use vstd::prelude::*;
verus! {
pub open spec fn mutated_opaque_authority_count_v1() -> nat { 0 }
pub proof fn mutated_opaque_failure_may_drop_authority_v1()
    ensures mutated_opaque_authority_count_v1() == 1, {}
}
