// Expected-negative R46 mutation: success omits the mandatory final N audit.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_final_audit_count_v1(request_count: nat) -> nat { 0 }
pub proof fn mutated_success_has_one_full_audit_v1(request_count: nat)
    requires request_count > 0,
    ensures mutated_final_audit_count_v1(request_count) == request_count,
{}
}
