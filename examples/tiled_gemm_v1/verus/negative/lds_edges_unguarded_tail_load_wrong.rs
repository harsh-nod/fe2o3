use vstd::prelude::*;

#[path = "../lds_tiled_edges_alpha_beta.rs"]
mod model;

verus! {

/// Mutation: lane 1 loads A row one for M=1 without checking its row predicate.
/// Its packed index is exactly one past the single-element A allocation.
pub proof fn mutated_unguarded_tail_load_is_in_bounds_v1()
    ensures model::edges_a_index_v1(0, 0, 1, 0, 1) < 1,
{
}

} // verus!
