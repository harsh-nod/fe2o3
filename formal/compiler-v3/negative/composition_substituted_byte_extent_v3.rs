#[path = "../guarded_u32_xor_helper_store_composition_v3.rs"]
mod composition;

use vstd::prelude::*;

verus! {

proof fn hostile_mutation_v3(gid: int, element_extent: int, byte_len: int)
    requires
        0 <= gid,
        gid < element_extent,
        byte_len == 2 * element_extent,
    ensures
        4 * gid + 4 <= byte_len,
{
}

}
