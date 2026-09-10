// Replacing actual padded backing cost with requested logical bytes undercharges.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cache_admission_v1(used: u64, logical: u64, backing: u64, cap: u64) -> bool {
    (used as int) + (logical as int) <= (cap as int)
}
pub proof fn mutated_logical_bytes_undercharge_v1(used: u64, logical: u64, backing: u64, cap: u64)
    requires 0 < logical < backing <= 206158430208, cap <= 206158430208,
        (used as int) + (logical as int) <= (cap as int),
        (used as int) + (backing as int) > (cap as int),
    ensures !mutated_cache_admission_v1(used, logical, backing, cap),
{}
}
