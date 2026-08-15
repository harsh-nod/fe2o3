use vstd::prelude::*;

#[path = "../lds_tiled_edges_alpha_beta.rs"]
mod model;

verus! {

/// Mutation: lane 1 stores output column one for N=1 without checking its C
/// predicate. Its packed index is exactly one past the one-element C output.
pub proof fn mutated_unguarded_tail_store_is_in_bounds_v1()
    ensures model::edges_c_index_v1(0, 0, 1, 0, 1) < 1,
{
}

} // verus!
