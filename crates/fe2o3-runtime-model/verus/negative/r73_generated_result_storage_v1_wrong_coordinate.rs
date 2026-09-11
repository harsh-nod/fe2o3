// Named mutation of the R73 result storage guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_charge_v1(bytes: u64) -> Seq<u64> {
    Seq::new(19, |i: int| if i == 13 { bytes } else { 0u64 })
}
pub proof fn mutated_wrong_coordinate_v1(bytes: u64)
    requires bytes > 0,
    ensures mutated_charge_v1(bytes)[14] == bytes,
{}
}
