use vstd::prelude::*;
verus! {
pub open spec fn mutated_partial_continuation_visible_v1() -> bool { true }
pub proof fn mutated_d2d_continuation_may_precede_full_retirement_v1()
    ensures !mutated_partial_continuation_visible_v1(), {}
}
