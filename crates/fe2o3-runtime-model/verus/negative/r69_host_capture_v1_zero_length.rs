// Removing the nonempty-range check admits an empty capture.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_range_v1(offset: u64, length: u64, allocation: u64, destination: u64) -> bool {
    length == destination
        && offset as int + length as int <= u64::MAX as int
        && offset as int + length as int <= allocation as int
}
pub proof fn mutated_zero_length_v1(offset: u64, length: u64, allocation: u64, destination: u64)
    requires length == 0, destination == 0, offset <= allocation,
    ensures !mutated_range_v1(offset, length, allocation, destination),
{}
}
