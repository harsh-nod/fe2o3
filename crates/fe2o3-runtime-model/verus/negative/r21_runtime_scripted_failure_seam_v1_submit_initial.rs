use vstd::prelude::*;
verus! {
pub open spec fn mutated_initial_retry_code_v1(_completed: nat) -> int { 0 }
pub proof fn mutated_initial_retry_is_conclusive_failure_v1()
    ensures mutated_initial_retry_code_v1(0) == -1, {}
}
