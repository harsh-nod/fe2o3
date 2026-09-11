// Named mutation of the R73 result storage guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_shape_v1(expected: u64, actual: u64, capacity: u64, access: bool) -> bool {
    actual == capacity && access
}
pub proof fn mutated_shape_length_omitted_v1(expected: u64, actual: u64)
    requires expected != actual,
    ensures !mutated_shape_v1(expected, actual, actual, true),
{}
}
