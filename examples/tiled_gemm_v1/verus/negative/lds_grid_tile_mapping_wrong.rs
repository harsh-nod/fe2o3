use vstd::prelude::*;

#[path = "../lds_tiled_grid_stride.rs"]
mod model;

verus! {

/// Mutation: every x workgroup is assigned tile column zero.
pub open spec fn mutated_grid_tile_col_v1(_group_x: nat) -> nat { 0 }

/// Expected failure marker: mutated_grid_mapping_is_injective_v1.
pub proof fn mutated_grid_mapping_is_injective_v1()
    ensures mutated_grid_tile_col_v1(0) != mutated_grid_tile_col_v1(1),
{
}

} // verus!
