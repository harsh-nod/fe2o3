// Weakening exact destination equality to >= admits unfilled trailing bytes.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_range_v1(offset: u64, length: u64, allocation: u64, destination: u64) -> bool {
    length > 0 && length <= destination
        && offset as int + length as int <= u64::MAX as int
        && offset as int + length as int <= allocation as int
}
pub proof fn mutated_long_destination_v1(offset: u64, length: u64, allocation: u64, destination: u64)
    requires 0 < length < destination, offset as int + length as int <= allocation as int,
    ensures !mutated_range_v1(offset, length, allocation, destination),
{}
}
