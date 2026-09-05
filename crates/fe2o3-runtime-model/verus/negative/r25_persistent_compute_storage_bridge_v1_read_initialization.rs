use vstd::prelude::*;
verus! {
pub open spec fn mutated_uninitialized_read_is_allowed_v1() -> bool { true }
pub proof fn mutated_reads_require_initialization_v1()
    ensures !mutated_uninitialized_read_is_allowed_v1(), {}
}
