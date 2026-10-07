// Keeping native generation checks does not replace the pool generation guard.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_entry_valid_v1(id: u64, native_generation: u64, pool_generation: u64, backing: u64) -> bool {
    id != 0 && native_generation != 0 && 0 < backing <= 206158430208
}
pub proof fn mutated_zero_pool_generation_v1(id: u64, native_generation: u64, pool_generation: u64, backing: u64)
    requires id != 0, native_generation != 0, pool_generation == 0, 0 < backing <= 206158430208,
    ensures !mutated_entry_valid_v1(id, native_generation, pool_generation, backing),
{}
}
