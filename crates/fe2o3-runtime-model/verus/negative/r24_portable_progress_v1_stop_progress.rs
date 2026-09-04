use vstd::prelude::*;
verus! {
pub open spec fn mutated_visits_before_stop_v1() -> nat { 7 }
pub open spec fn mutated_visits_after_stop_v1() -> nat { 8 }
pub proof fn mutated_stop_performs_no_final_progress_v1()
    ensures mutated_visits_before_stop_v1() == mutated_visits_after_stop_v1(), {}
}
