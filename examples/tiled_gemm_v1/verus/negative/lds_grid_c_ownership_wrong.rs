use vstd::prelude::*;

#[path = "../lds_tiled_grid_stride.rs"]
mod model;

verus! {

/// Mutation: C addressing drops group_x, so corresponding owners in adjacent
/// output tiles collide.
pub open spec fn mutated_c_index_without_group_x_v1(
    _group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    ldc: nat,
) -> nat {
    model::grid_c_row_v1(group_y, lane, component) * ldc
        + model::grid_c_col_v1(0, lane)
}

pub proof fn mutated_distinct_grid_owners_have_disjoint_c_v1()
    ensures
        mutated_c_index_without_group_x_v1(0, 0, 0, 0, 32)
            != mutated_c_index_without_group_x_v1(1, 0, 0, 0, 32),
{
}

} // verus!
