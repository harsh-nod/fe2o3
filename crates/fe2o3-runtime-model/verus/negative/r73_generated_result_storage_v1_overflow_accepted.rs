// Named mutation of the R73 result storage guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_admitted_v1(bytes: u64) -> bool { true }
pub proof fn mutated_overflow_accepted_v1(bytes: u64)
    requires bytes > u64::MAX / 2,
    ensures !mutated_admitted_v1(bytes),
{}
}
