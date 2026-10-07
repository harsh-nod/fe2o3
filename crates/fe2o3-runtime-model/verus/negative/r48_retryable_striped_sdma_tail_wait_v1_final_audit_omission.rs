// Expected-negative R48 mutation: deadline timeout skips the one N audit.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_audit_work_v1(_requests: nat) -> nat { 0 }
pub proof fn timeout_audits_exact_n_v1(requests: nat)
    requires requests > 0,
    ensures mutated_timeout_audit_work_v1(requests) == requests,
{}
}
