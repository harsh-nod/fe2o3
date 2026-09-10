// A backing-only debit omits the native allocation record coordinate.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_projection_v1(backing: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 3 { backing } else { 0u64 })
}
pub proof fn mutated_allocation_record_omitted_v1(backing: u64)
    requires 0 < backing <= 206158430208,
    ensures mutated_projection_v1(backing)[18] == 1,
{}
}
