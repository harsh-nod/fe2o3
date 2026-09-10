// Byte capacity does not authorize one more record after the record cap is full.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cache_admission_v1(used: u64, incoming: u64, byte_cap: u64, count: usize, record_cap: usize) -> bool {
    (used as int) + (incoming as int) <= (byte_cap as int)
}
pub proof fn mutated_record_limit_omitted_v1(used: u64, incoming: u64, byte_cap: u64, count: usize, record_cap: usize)
    requires incoming != 0, byte_cap <= 206158430208,
        (used as int) + (incoming as int) <= (byte_cap as int),
        count == record_cap, record_cap <= 128,
    ensures !mutated_cache_admission_v1(used, incoming, byte_cap, count, record_cap),
{}
}
