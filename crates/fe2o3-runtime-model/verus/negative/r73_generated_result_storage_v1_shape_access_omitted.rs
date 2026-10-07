// Named mutation of the R73 result storage guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_shape_v1(expected: u64, actual: u64, capacity: u64, access: bool) -> bool {
    expected == actual && actual == capacity
}
pub proof fn mutated_shape_access_omitted_v1(bytes: u64)
    ensures !mutated_shape_v1(bytes, bytes, bytes, false),
{}
}
