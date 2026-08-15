use vstd::prelude::*;

#[path = "../lds_tiled_slice1.rs"]
mod model;

verus! {

/// Mutation: two distinct physical invocations are claimed to own the same C
/// element by dropping the second owner's lane identity.
pub proof fn mutated_distinct_source_owners_may_alias_v1()
    ensures model::c_global_index_v1(0, 0) == model::c_global_index_v1(1, 0),
{
}

} // verus!
