// Replacing checked addition with u64 wrapping admits overflowed ranges.
use vstd::prelude::*;
verus! {
pub open spec fn wrapped_end_v1(offset: u64, length: u64) -> int {
    let end = offset as int + length as int;
    if end > u64::MAX as int { end - 18446744073709551616 } else { end }
}
pub open spec fn mutated_range_v1(offset: u64, length: u64, allocation: u64, destination: u64) -> bool {
    length > 0 && length == destination && wrapped_end_v1(offset, length) <= allocation as int
}
pub proof fn mutated_overflow_v1(offset: u64, length: u64, allocation: u64, destination: u64)
    requires
        length > 0, length == destination,
        offset as int + length as int > u64::MAX as int,
        wrapped_end_v1(offset, length) <= allocation as int,
    ensures !mutated_range_v1(offset, length, allocation, destination),
{}
}
