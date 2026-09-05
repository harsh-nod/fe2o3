use vstd::prelude::*;
verus! {
pub open spec fn published_generation_v1() -> nat { 7 }
pub open spec fn mutated_pending_generation_v1() -> nat { 0 }
pub proof fn mutated_pending_retains_exact_custody_v1()
    ensures mutated_pending_generation_v1() == published_generation_v1(), {}
}
