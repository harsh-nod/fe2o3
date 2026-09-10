use vstd::prelude::*;
verus! {
pub open spec fn acquire_v1(terminal: bool, reserved: bool, submissions: nat, events: nat) -> bool {
    !terminal && submissions == 0 && events == 0
}
pub open spec fn issue_v1(terminal: bool, exact: bool, closed: bool) -> bool {
    !terminal && exact && !closed
}
pub open spec fn release_v1(terminal: bool, exact: bool, closed: bool, submissions: nat, events: nat) -> bool {
    !terminal && exact && closed && submissions == 0 && events == 0
}
pub proof fn mutated_acquire_reserved_v1(t: bool, r: bool, s: nat, e: nat)
    requires acquire_v1(t, r, s, e),
    ensures !t && !r,
{}
}
