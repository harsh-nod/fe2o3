// A free record alone cannot authorize caching beyond the padded-byte ceiling.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_cache_admission_v1(used: u64, incoming: u64, byte_cap: u64, count: usize, record_cap: usize) -> bool {
    (count as int) + 1 <= (record_cap as int)
}
pub proof fn mutated_byte_limit_omitted_v1(used: u64, incoming: u64, byte_cap: u64, count: usize, record_cap: usize)
    requires used <= byte_cap <= 206158430208, 0 < incoming <= 206158430208,
        (used as int) + (incoming as int) > (byte_cap as int),
        count < record_cap <= 128,
    ensures !mutated_cache_admission_v1(used, incoming, byte_cap, count, record_cap),
{}
}
