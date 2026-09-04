use vstd::prelude::*;

verus! {
pub open spec fn mutated_max_allocation_bytes_v1() -> nat { 1024 * 1024 * 1024 }
pub proof fn mutated_one_gib_allocation_is_rejected_v1()
    ensures mutated_max_allocation_bytes_v1() == 256 * 1024 * 1024,
{}
}
