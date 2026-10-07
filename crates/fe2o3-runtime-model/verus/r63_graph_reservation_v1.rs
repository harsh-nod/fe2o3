// Abstract reservation guards only; ledger, identity privacy and backend refinement are external.
use vstd::prelude::*;
verus! {
pub open spec fn acquire_v1(terminal: bool, reserved: bool, submissions: nat, events: nat) -> bool {
    !terminal && !reserved && submissions == 0 && events == 0
}
pub open spec fn issue_v1(terminal: bool, exact: bool, closed: bool) -> bool {
    !terminal && exact && !closed
}
pub open spec fn release_v1(terminal: bool, exact: bool, closed: bool, submissions: nat, events: nat) -> bool {
    !terminal && exact && closed && submissions == 0 && events == 0
}
pub proof fn clean_acquisition_v1(t: bool, r: bool, s: nat, e: nat)
    requires acquire_v1(t, r, s, e),
    ensures s == 0 && e == 0,
{}
pub proof fn exclusive_acquisition_v1(t: bool, r: bool, s: nat, e: nat)
    requires acquire_v1(t, r, s, e),
    ensures !t && !r,
{}
pub proof fn exact_issue_token_v1(t: bool, x: bool, c: bool)
    requires issue_v1(t, x, c),
    ensures x,
{}
pub proof fn closed_issue_v1(t: bool, x: bool)
    ensures !issue_v1(t, x, true),
{}
pub proof fn empty_release_v1(t: bool, x: bool, c: bool, s: nat, e: nat)
    requires release_v1(t, x, c, s, e),
    ensures s == 0 && e == 0,
{}
pub proof fn closed_release_v1(t: bool, x: bool, c: bool, s: nat, e: nat)
    requires release_v1(t, x, c, s, e),
    ensures c,
{}
pub proof fn terminal_retention_v1(x: bool, c: bool, s: nat, e: nat)
    ensures !release_v1(true, x, c, s, e),
{}
pub proof fn exact_release_token_v1(t: bool, x: bool, c: bool, s: nat, e: nat)
    requires release_v1(t, x, c, s, e),
    ensures x,
{}
}
