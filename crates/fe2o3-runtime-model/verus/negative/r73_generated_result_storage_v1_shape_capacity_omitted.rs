// Named mutation of the R73 result storage guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_shape_v1(expected: u64, actual: u64, capacity: u64, access: bool) -> bool {
    expected == actual && access
}
pub proof fn mutated_shape_capacity_omitted_v1(actual: u64, capacity: u64)
    requires actual != capacity,
    ensures !mutated_shape_v1(actual, actual, capacity, true),
{}
}
