use vstd::prelude::*;
verus! {
pub open spec fn acquire_v1(terminal: bool, reserved: bool, submissions: nat, events: nat) -> bool {
    !terminal && !reserved && submissions == 0 && events == 0
}
pub open spec fn issue_v1(terminal: bool, exact: bool, closed: bool) -> bool {
    !terminal && exact
}
pub open spec fn release_v1(terminal: bool, exact: bool, closed: bool, submissions: nat, events: nat) -> bool {
    !terminal && exact && closed && submissions == 0 && events == 0
}
pub proof fn mutated_issue_after_close_v1(t: bool, x: bool)
    ensures !issue_v1(t, x, true),
{}
}
