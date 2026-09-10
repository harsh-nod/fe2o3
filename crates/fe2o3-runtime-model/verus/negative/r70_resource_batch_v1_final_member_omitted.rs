// Stopping before the final member admits a two-member roster whose sum fails.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_roster_admission_v1(used: u64, first: u64, last: u64, capacity: u64) -> bool {
    (used as int) + (first as int) <= (capacity as int)
}
pub proof fn mutated_final_member_omitted_v1(used: u64, first: u64, last: u64, capacity: u64)
    requires
        (used as int) + (first as int) <= (capacity as int),
        (used as int) + (first as int) + (last as int) > (capacity as int),
    ensures !mutated_roster_admission_v1(used, first, last, capacity),
{}
}
