use vstd::prelude::*;

#[path = "../lds_tiled_slice1.rs"]
mod model;

verus! {

/// Expected failure: the proved matrix-product correspondence cannot justify
/// an additional unit in every output accumulator.
pub proof fn mutated_lds_result_has_extra_unit_v1(
    a: Seq<real>,
    b: Seq<real>,
    epoch: nat,
)
    requires model::fixed_tile_inputs_v1(a, b),
    ensures
        model::lds_dot_prefix_v1(a, b, 0, 0, 16)
            == model::global_dot_prefix_v1(a, b, 0, 0, 16) + 1real,
{
    model::fixed_tile_lds_result_is_matrix_product_v1(a, b, epoch, 0, 0);
}

} // verus!
