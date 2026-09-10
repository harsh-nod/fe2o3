// Wrapping the second prefix can admit a sum larger than the u64 domain.
use vstd::prelude::*;
verus! {
pub open spec fn wrapped_second_prefix_v1(used: u64, first: u64, last: u64) -> int {
    let sum = (used as int) + (first as int) + (last as int);
    if sum > (u64::MAX as int) { sum - 18446744073709551616 } else { sum }
}
pub open spec fn mutated_roster_admission_v1(used: u64, first: u64, last: u64, capacity: u64) -> bool {
    (used as int) + (first as int) <= (capacity as int)
        && wrapped_second_prefix_v1(used, first, last) <= (capacity as int)
}
pub proof fn mutated_aggregate_overflow_v1(used: u64, first: u64, last: u64, capacity: u64)
    requires
        (used as int) + (first as int) <= (capacity as int),
        (used as int) + (first as int) + (last as int) > (u64::MAX as int),
        wrapped_second_prefix_v1(used, first, last) <= (capacity as int),
    ensures !mutated_roster_admission_v1(used, first, last, capacity),
{}
}
