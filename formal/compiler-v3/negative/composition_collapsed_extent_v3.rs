#[path = "../guarded_u32_xor_helper_store_composition_v3.rs"]
mod composition;

use vstd::prelude::*;

verus! {

proof fn hostile_mutation_v3(gid: int, first: int, missing: int, output: int)
    ensures
        composition::ordered_guard_v3(gid, seq![first, first, output])
            ==> gid < missing,
{
}

}
