use vstd::prelude::*;

verus! {
pub open spec fn mutated_can_release_v1(current: bool, terminal: bool) -> bool { terminal }
pub proof fn mutated_quarantined_terminal_is_unreleasable_v1()
    ensures !mutated_can_release_v1(false, true),
{}
}
