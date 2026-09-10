// Dropping bounded costs and replacing checked addition with wrapping addition.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cache_admission_v1(used: u64, incoming: u64, cap: u64) -> bool {
    ((used as int) + (incoming as int)) % 18446744073709551616 <= (cap as int)
}
pub proof fn mutated_unbounded_wrapping_sum_v1(used: u64, incoming: u64, cap: u64)
    requires used <= cap, incoming != 0,
        (used as int) + (incoming as int) > (u64::MAX as int),
        mutated_cache_admission_v1(used, incoming, cap),
    ensures (used as int) + (incoming as int) <= (cap as int),
{}
}
