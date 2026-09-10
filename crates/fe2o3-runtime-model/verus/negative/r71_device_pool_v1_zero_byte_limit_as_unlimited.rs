// Zero cache limits mean disabled, not the legacy unconfigured policy.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cache_admission_v1(incoming: u64, byte_cap: u64, record_cap: usize) -> bool {
    (byte_cap == 0 || incoming <= byte_cap) && record_cap > 0
}
pub proof fn mutated_zero_byte_limit_as_unlimited_v1(incoming: u64, byte_cap: u64, record_cap: usize)
    requires 0 < incoming <= 206158430208, byte_cap == 0, 0 < record_cap <= 128,
    ensures !mutated_cache_admission_v1(incoming, byte_cap, record_cap),
{}
}
