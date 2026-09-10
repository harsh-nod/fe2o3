// Removing the hard member bound admits an otherwise fitting oversized roster.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_count_admission_v1(count: usize, free: usize, owner: u64) -> bool {
    count > 0 && count <= free && owner > 0
        && (owner as int) + (count as int) <= (u64::MAX as int)
}
pub proof fn mutated_oversized_roster_v1(count: usize, free: usize, owner: u64)
    requires count > 65536, count <= free, owner > 0,
        (owner as int) + (count as int) <= (u64::MAX as int),
    ensures !mutated_count_admission_v1(count, free, owner),
{}
}
