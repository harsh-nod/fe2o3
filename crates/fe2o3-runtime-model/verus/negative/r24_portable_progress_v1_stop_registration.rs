use vstd::prelude::*;
verus! {
pub open spec fn mutated_history_before_stop_v1() -> nat { 3 }
pub open spec fn mutated_history_after_stop_v1() -> nat { 2 }
pub proof fn mutated_stop_preserves_registration_history_v1()
    ensures mutated_history_before_stop_v1() == mutated_history_after_stop_v1(), {}
}
