// Expected-negative R46 mutation: deadline timeout returns before the N audit.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_audit_count_v1(deadline_reached: bool) -> nat { 0 }
pub proof fn mutated_timeout_before_audit_is_rejected_v1(request_count: nat)
    requires request_count > 0,
    ensures mutated_timeout_audit_count_v1(true) == request_count,
{}
}
