// Expected-negative R48 mutation: an audit error prefix is counted as a full N audit.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_audit_error_work_v1(requests: nat, _prefix: nat) -> nat { requests }
pub proof fn audit_error_stops_at_prefix_v1()
    ensures mutated_audit_error_work_v1(17, 5) == 5,
{}
}
