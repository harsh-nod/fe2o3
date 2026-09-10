// A second physical host charge invents backing not present in the N2 layout.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_projection_v1(backing: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 2 || i == 3 { backing } else if i == 18 { 1u64 } else { 0u64 })
}
pub proof fn mutated_host_double_charge_v1(backing: u64)
    requires 0 < backing <= 206158430208,
    ensures mutated_projection_v1(backing)[2] == 0,
{}
}
