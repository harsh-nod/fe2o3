use vstd::prelude::*;
verus! {
pub open spec fn storage_bytes_v1() -> nat { 4096 }
pub open spec fn mutated_logical_bytes_v1() -> nat { 4095 }
pub proof fn mutated_logical_and_physical_ranges_are_exact_v1()
    ensures mutated_logical_bytes_v1() == storage_bytes_v1(), {}
}
