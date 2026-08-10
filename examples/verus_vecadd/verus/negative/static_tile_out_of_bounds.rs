use vstd::prelude::*;

#[path = "../static_tile.rs"]
mod static_tile_model;

verus! {

/// Expected failure marker: mutated_static_tile_out_of_bounds_is_safe.
pub proof fn mutated_static_tile_out_of_bounds_is_safe(
    witness: static_tile_model::StaticTileWitness,
)
    requires
        static_tile_model::tile_witness_is_checked(witness),
    ensures
        static_tile_model::allocation_relative_index(witness, witness.tile_len)
            < witness.parent.start + witness.parent.len,
{
}

} // verus!
