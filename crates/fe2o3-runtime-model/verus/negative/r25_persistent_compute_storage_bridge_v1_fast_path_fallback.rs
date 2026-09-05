use vstd::prelude::*;
verus! {
pub open spec fn mutated_fallback_after_selection_v1() -> bool { true }
pub proof fn mutated_selected_fast_path_has_no_fallback_v1()
    ensures !mutated_fallback_after_selection_v1(), {}
}
