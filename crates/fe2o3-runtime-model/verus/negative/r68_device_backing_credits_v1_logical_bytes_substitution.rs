// Charging the requested logical extent omits the native backing padding.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_projection_v1(requested: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 3 { requested } else if i == 18 { 1u64 } else { 0u64 })
}
pub proof fn mutated_logical_bytes_substitution_v1(requested: u64, backing: u64)
    requires 0 < requested < backing <= 206158430208,
    ensures mutated_projection_v1(requested)[3] == backing,
{}
}
