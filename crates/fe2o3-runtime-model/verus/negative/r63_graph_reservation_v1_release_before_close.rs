use vstd::prelude::*;
verus! {
pub open spec fn acquire_v1(terminal: bool, reserved: bool, submissions: nat, events: nat) -> bool {
    !terminal && !reserved && submissions == 0 && events == 0
}
pub open spec fn issue_v1(terminal: bool, exact: bool, closed: bool) -> bool {
    !terminal && exact && !closed
}
pub open spec fn release_v1(terminal: bool, exact: bool, closed: bool, submissions: nat, events: nat) -> bool {
    !terminal && exact && submissions == 0 && events == 0
}
pub proof fn mutated_release_before_close_v1(t: bool, x: bool, c: bool, s: nat, e: nat)
    requires release_v1(t, x, c, s, e),
    ensures c,
{}
}
