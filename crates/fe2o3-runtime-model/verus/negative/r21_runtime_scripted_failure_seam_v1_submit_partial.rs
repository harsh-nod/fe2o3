use vstd::prelude::*;
verus! {
pub open spec fn mutated_partial_retry_host_dirty_v1(_host_dirty: nat) -> nat { 0 }
pub proof fn mutated_partial_host_mutation_retry_preserves_exact_progress_v1()
    ensures mutated_partial_retry_host_dirty_v1(4194272) == 4194272, {}
}
