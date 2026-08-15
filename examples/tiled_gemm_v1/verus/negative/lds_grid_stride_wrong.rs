use vstd::prelude::*;

#[path = "../lds_tiled_grid_stride.rs"]
mod model;

verus! {

/// Mutation: M=32 accepts lda=15 and allocates only 32*15 BF16 elements.
/// Lane 63 component 3 in workgroup row one then addresses element 480,
/// exactly one past that allocation.
pub proof fn mutated_undersized_lda_keeps_a_load_in_bounds_v1()
    ensures model::grid_a_index_v1(1, 63, 3, 15) < 32 * 15,
{
}

} // verus!
