use vstd::prelude::*;
verus! {
pub open spec fn mutated_restore_v1(expected_status: nat, observed_status: nat) -> bool { true }
pub proof fn mutated_terminal_status_substitution_is_rejected_v1()
    ensures !mutated_restore_v1(7, 8),
{}
}
