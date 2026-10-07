use vstd::prelude::*;
verus! {
// Mutation: terminal observation returns the prior progress-registration bit
// instead of retiring it.
pub open spec fn mutated_terminal_progress_registration_v1(active_before: bool) -> bool {
    active_before
}
pub proof fn mutated_terminal_retires_progress_registration_v1()
    ensures !mutated_terminal_progress_registration_v1(true), {}
}
