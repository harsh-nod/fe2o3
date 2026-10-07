// Removing the nonempty bound admits a roster that owns no member record.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_count_admission_v1(count: usize, free: usize, owner: u64) -> bool {
    count <= 65536 && count <= free && owner > 0
        && (owner as int) + (count as int) <= (u64::MAX as int)
}
pub proof fn mutated_empty_roster_v1(count: usize, free: usize, owner: u64)
    requires count == 0, owner > 0,
    ensures !mutated_count_admission_v1(count, free, owner),
{}
}
