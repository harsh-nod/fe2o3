use vstd::prelude::*;
verus! {
pub open spec fn quarantined_v1() -> bool { true }
pub open spec fn mutated_post_retention_quarantined_v1() -> bool { false }
pub proof fn mutated_post_retention_fault_quarantines_v1()
    ensures mutated_post_retention_quarantined_v1() == quarantined_v1(), {}
}
