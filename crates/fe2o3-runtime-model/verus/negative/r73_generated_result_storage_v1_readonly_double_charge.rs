// Named mutation of the R73 result storage guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_peak_v1(bytes: u64) -> int { 2 * (bytes as int) }
pub proof fn mutated_readonly_double_charge_v1(bytes: u64)
    requires 0 < bytes <= u64::MAX / 2,
    ensures mutated_peak_v1(bytes) == bytes as int,
{}
}
