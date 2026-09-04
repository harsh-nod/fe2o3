use vstd::prelude::*;
verus! {
pub open spec fn mutated_teardown_release_authority_v1(_prior: nat) -> nat { 0 }
pub proof fn mutated_teardown_blocks_release_v1()
    ensures mutated_teardown_release_authority_v1(1) == 1, {}
}
