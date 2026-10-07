use vstd::prelude::*;
verus! {
pub open spec fn mutated_terminal_stage_before_v1() -> nat { 2 }
pub open spec fn mutated_terminal_stage_after_v1() -> nat { 1 }
pub proof fn mutated_terminal_quarantine_is_absorbing_v1()
    ensures mutated_terminal_stage_before_v1() == mutated_terminal_stage_after_v1(), {}
}
