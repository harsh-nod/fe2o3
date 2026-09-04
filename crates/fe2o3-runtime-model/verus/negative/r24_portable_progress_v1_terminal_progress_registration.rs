use vstd::prelude::*;
verus! {
pub open spec fn mutated_terminal_progress_registration_active_v1() -> bool { true }
pub proof fn mutated_terminal_retires_progress_registration_v1()
    ensures !mutated_terminal_progress_registration_active_v1(), {}
}
