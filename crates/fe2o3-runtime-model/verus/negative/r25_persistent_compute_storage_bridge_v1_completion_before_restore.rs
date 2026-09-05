use vstd::prelude::*;
verus! {
pub open spec fn mutated_restore_before_completion_v1() -> bool { true }
pub proof fn mutated_restore_requires_completion_v1()
    ensures !mutated_restore_before_completion_v1(), {}
}
