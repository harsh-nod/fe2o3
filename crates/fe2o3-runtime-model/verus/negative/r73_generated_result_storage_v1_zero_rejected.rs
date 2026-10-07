// Named mutation of the R73 result storage guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_admitted_v1(bytes: u64) -> bool { bytes > 0 && bytes <= u64::MAX / 2 }
pub proof fn mutated_zero_rejected_v1()
    ensures mutated_admitted_v1(0),
{}
}
