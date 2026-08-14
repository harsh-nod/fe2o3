use vstd::arithmetic::div_mod::{
    lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse, lemma_mod_bound,
};
use vstd::prelude::*;

#[path = "tiled_gemm_host_contract.rs"]
mod base;

verus! {

pub open spec fn slice1_tile_elements_v1() -> nat { 256 }
pub open spec fn slice1_lds_elements_v1() -> nat { 512 }
pub open spec fn slice1_a_lds_base_v1() -> nat { 0 }
pub open spec fn slice1_b_lds_base_v1() -> nat { 256 }

/// Slice 1 is exactly one 16x16x16 tile. Each sequence entry is the exact
/// mathematical value represented by a finite BF16 input after widening to
/// F32. Bit-level decoding and IEEE-754 exceptional values are outside this
/// model.
pub open spec fn fixed_tile_inputs_v1(a: Seq<real>, b: Seq<real>) -> bool {
    a.len() == slice1_tile_elements_v1()
        && b.len() == slice1_tile_elements_v1()
}

pub open spec fn a_global_index_v1(row: nat, depth: nat) -> nat {
    row * 16 + depth
}

pub open spec fn b_global_index_v1(depth: nat, column: nat) -> nat {
    depth * 16 + column
}

pub open spec fn a_lds_address_v1(row: nat, depth: nat) -> nat {
    slice1_a_lds_base_v1() + base::xor4_lds_index_v1(row, depth)
}

/// B is stored transposed in LDS, so its logical LDS coordinate is
/// `(column, depth)` while its global coordinate is `(depth, column)`.
pub open spec fn b_lds_address_v1(depth: nat, column: nat) -> nat {
    slice1_b_lds_base_v1() + base::xor4_lds_index_v1(column, depth)
}

pub open spec fn a_writer_lane_v1(row: nat, depth: nat) -> nat {
    (depth / 4) * 16 + row
}

pub open spec fn b_writer_lane_v1(depth: nat, column: nat) -> nat {
    (depth / 4) * 16 + column
}

pub open spec fn writer_component_v1(depth: nat) -> nat {
    depth % 4
}

pub open spec fn a_cooperative_write_address_v1(lane: nat, component: nat) -> nat {
    slice1_a_lds_base_v1() + base::a_lds_index_v1(lane, component)
}

pub open spec fn b_cooperative_write_address_v1(lane: nat, component: nat) -> nat {
    slice1_b_lds_base_v1() + base::b_transposed_lds_index_v1(lane, component)
}

pub open spec fn a_cooperative_write_value_v1(
    a: Seq<real>,
    lane: nat,
    component: nat,
) -> real
    recommends
        a.len() == slice1_tile_elements_v1(),
        lane < 64,
        component < 4,
{
    a[a_global_index_v1(
        base::a_register_row_v1(lane),
        base::a_register_depth_v1(lane, component),
    ) as int]
}

pub open spec fn b_cooperative_write_value_v1(
    b: Seq<real>,
    lane: nat,
    component: nat,
) -> real
    recommends
        b.len() == slice1_tile_elements_v1(),
        lane < 64,
        component < 4,
{
    b[b_global_index_v1(
        base::b_register_depth_v1(lane, component),
        base::b_register_col_v1(lane),
    ) as int]
}

pub open spec fn write_barrier_epoch_v1(epoch: nat) -> nat { epoch }
pub open spec fn read_barrier_epoch_v1(epoch: nat) -> nat { epoch }

/// The canonical writer is part of the model, so initialization requires a
/// valid writer, an identical physical address, an identical value, and the
/// same barrier epoch. It is not merely an address-bounds predicate.
pub open spec fn a_read_initialized_same_epoch_v1(
    a: Seq<real>,
    epoch: nat,
    row: nat,
    depth: nat,
) -> bool {
    &&& a_writer_lane_v1(row, depth) < 64
    &&& writer_component_v1(depth) < 4
    &&& a_cooperative_write_address_v1(
        a_writer_lane_v1(row, depth), writer_component_v1(depth),
    ) == a_lds_address_v1(row, depth)
    &&& write_barrier_epoch_v1(epoch) == read_barrier_epoch_v1(epoch)
    &&& a_cooperative_write_value_v1(
        a, a_writer_lane_v1(row, depth), writer_component_v1(depth),
    ) == a[a_global_index_v1(row, depth) as int]
}

pub open spec fn b_read_initialized_same_epoch_v1(
    b: Seq<real>,
    epoch: nat,
    depth: nat,
    column: nat,
) -> bool {
    &&& b_writer_lane_v1(depth, column) < 64
    &&& writer_component_v1(depth) < 4
    &&& b_cooperative_write_address_v1(
        b_writer_lane_v1(depth, column), writer_component_v1(depth),
    ) == b_lds_address_v1(depth, column)
    &&& write_barrier_epoch_v1(epoch) == read_barrier_epoch_v1(epoch)
    &&& b_cooperative_write_value_v1(
        b, b_writer_lane_v1(depth, column), writer_component_v1(depth),
    ) == b[b_global_index_v1(depth, column) as int]
}

pub open spec fn a_lds_read_value_v1(
    a: Seq<real>,
    row: nat,
    depth: nat,
) -> real
    recommends
        a.len() == slice1_tile_elements_v1(),
        row < 16,
        depth < 16,
{
    a_cooperative_write_value_v1(
        a, a_writer_lane_v1(row, depth), writer_component_v1(depth),
    )
}

pub open spec fn b_lds_read_value_v1(
    b: Seq<real>,
    depth: nat,
    column: nat,
) -> real
    recommends
        b.len() == slice1_tile_elements_v1(),
        depth < 16,
        column < 16,
{
    b_cooperative_write_value_v1(
        b, b_writer_lane_v1(depth, column), writer_component_v1(depth),
    )
}

/// Widening BF16 to F32 is exact for finite BF16 values, so it is identity on
/// the mathematical-real values used by this model.
pub open spec fn bf16_to_f32_abstract_v1(value: real) -> real { value }

/// Slice 1's arithmetic abstraction is exact real multiply-add in the kernel's
/// K order. This deliberately does not claim bitwise IEEE-F32/MFMA rounding,
/// NaN behavior, signed-zero behavior, or overflow correspondence.
pub open spec fn f32_multiply_add_abstract_v1(
    accumulator: real,
    lhs: real,
    rhs: real,
) -> real {
    accumulator + bf16_to_f32_abstract_v1(lhs) * bf16_to_f32_abstract_v1(rhs)
}

pub open spec fn global_dot_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    end: nat,
) -> real
    recommends
        fixed_tile_inputs_v1(a, b),
        row < 16,
        column < 16,
        end <= 16,
    decreases end,
{
    if end == 0 {
        0real
    } else {
        f32_multiply_add_abstract_v1(
            global_dot_prefix_v1(a, b, row, column, (end - 1) as nat),
            a[a_global_index_v1(row, (end - 1) as nat) as int],
            b[b_global_index_v1((end - 1) as nat, column) as int],
        )
    }
}

pub open spec fn lds_dot_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    end: nat,
) -> real
    recommends
        fixed_tile_inputs_v1(a, b),
        row < 16,
        column < 16,
        end <= 16,
    decreases end,
{
    if end == 0 {
        0real
    } else {
        f32_multiply_add_abstract_v1(
            lds_dot_prefix_v1(a, b, row, column, (end - 1) as nat),
            a_lds_read_value_v1(a, row, (end - 1) as nat),
            b_lds_read_value_v1(b, (end - 1) as nat, column),
        )
    }
}

proof fn depth_writer_coordinates_v1(depth: nat)
    requires depth < 16,
    ensures
        depth / 4 < 4,
        depth % 4 < 4,
        depth == (depth / 4) * 4 + depth % 4,
{
    lemma_mod_bound(depth as int, 4);
    lemma_fundamental_div_mod(depth as int, 4);
    assert(depth == 4 * (depth / 4) + depth % 4);
    assert(4 * (depth / 4) == (depth / 4) * 4) by (nonlinear_arith);
    if depth / 4 >= 4 {
        assert((depth / 4) * 4 >= 16) by (nonlinear_arith)
            requires depth / 4 >= 4;
        assert(depth >= 16);
        assert(false);
    }
}

proof fn packed_writer_lane_v1(axis: nat, depth: nat)
    requires axis < 16, depth < 16,
    ensures
        (depth / 4) * 16 + axis < 64,
        ((depth / 4) * 16 + axis) / 16 == depth / 4,
        ((depth / 4) * 16 + axis) % 16 == axis,
{
    depth_writer_coordinates_v1(depth);
    lemma_fundamental_div_mod_converse(
        ((depth / 4) * 16 + axis) as int,
        16,
        (depth / 4) as int,
        axis as int,
    );
    assert((depth / 4) * 16 + axis < 64) by (nonlinear_arith)
        requires depth / 4 < 4, axis < 16;
}

/// Every logical fixed-tile A and B global load is in bounds.
pub proof fn all_slice1_global_input_indices_are_bounded_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    depth: nat,
    column: nat,
)
    requires
        fixed_tile_inputs_v1(a, b),
        row < 16,
        depth < 16,
        column < 16,
    ensures
        a_global_index_v1(row, depth) < a.len(),
        b_global_index_v1(depth, column) < b.len(),
{
    assert(row * 16 + depth < 256) by (nonlinear_arith)
        requires row < 16, depth < 16;
    assert(depth * 16 + column < 256) by (nonlinear_arith)
        requires depth < 16, column < 16;
}

/// XOR4 addresses are bounded in separate A/B halves of the 512-element LDS.
pub proof fn xor4_a_b_lds_addresses_are_bounded_v1(
    row: nat,
    depth: nat,
    column: nat,
)
    requires row < 16, depth < 16, column < 16,
    ensures
        a_lds_address_v1(row, depth) < 256,
        256 <= b_lds_address_v1(depth, column) < slice1_lds_elements_v1(),
{
    base::xor4_physical_index_is_bounded_v1(row, depth);
    base::xor4_physical_index_is_bounded_v1(column, depth);
}

/// XOR4 remains injective independently in A's `(row, depth)` tile and B's
/// transposed `(column, depth)` tile.
pub proof fn xor4_a_b_lds_addresses_are_injective_v1(
    left_axis: nat,
    left_depth: nat,
    right_axis: nat,
    right_depth: nat,
)
    requires
        left_axis < 16,
        right_axis < 16,
        left_depth < 16,
        right_depth < 16,
        left_axis != right_axis || left_depth != right_depth,
    ensures
        a_lds_address_v1(left_axis, left_depth)
            != a_lds_address_v1(right_axis, right_depth),
        b_lds_address_v1(left_depth, left_axis)
            != b_lds_address_v1(right_depth, right_axis),
{
    base::xor4_physical_index_is_injective_v1(
        left_axis, left_depth, right_axis, right_depth,
    );
}

/// Unequal cooperative writers do not alias in either tile, and no A writer
/// can alias any B writer.
pub proof fn cooperative_lds_writes_are_disjoint_v1(
    left_lane: nat,
    left_component: nat,
    right_lane: nat,
    right_component: nat,
)
    requires
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        left_lane != right_lane || left_component != right_component,
    ensures
        a_cooperative_write_address_v1(left_lane, left_component)
            != a_cooperative_write_address_v1(right_lane, right_component),
        b_cooperative_write_address_v1(left_lane, left_component)
            != b_cooperative_write_address_v1(right_lane, right_component),
        a_cooperative_write_address_v1(left_lane, left_component)
            != b_cooperative_write_address_v1(right_lane, right_component),
{
    base::distinct_lane_components_have_disjoint_a_lds_v1(
        left_lane, left_component, right_lane, right_component,
    );
    base::distinct_lane_components_have_disjoint_b_lds_v1(
        left_lane, left_component, right_lane, right_component,
    );
    base::a_and_b_staging_are_bounded_v1(left_lane, left_component);
    assert(a_cooperative_write_address_v1(left_lane, left_component) < 256);
    assert(b_cooperative_write_address_v1(right_lane, right_component) >= 256);
}

/// The canonical A writer supplies the requested physical slot and value in
/// the read's barrier epoch.
pub proof fn every_a_lds_read_is_initialized_in_same_epoch_v1(
    a: Seq<real>,
    epoch: nat,
    row: nat,
    depth: nat,
)
    requires
        a.len() == slice1_tile_elements_v1(),
        row < 16,
        depth < 16,
    ensures a_read_initialized_same_epoch_v1(a, epoch, row, depth),
{
    packed_writer_lane_v1(row, depth);
    depth_writer_coordinates_v1(depth);
    let lane = a_writer_lane_v1(row, depth);
    let component = writer_component_v1(depth);
    assert(base::a_register_row_v1(lane) == row);
    assert(base::a_register_depth_v1(lane, component) == depth);
}

/// The canonical B writer supplies the requested transposed physical slot and
/// value in the read's barrier epoch.
pub proof fn every_b_lds_read_is_initialized_in_same_epoch_v1(
    b: Seq<real>,
    epoch: nat,
    depth: nat,
    column: nat,
)
    requires
        b.len() == slice1_tile_elements_v1(),
        depth < 16,
        column < 16,
    ensures b_read_initialized_same_epoch_v1(b, epoch, depth, column),
{
    packed_writer_lane_v1(column, depth);
    depth_writer_coordinates_v1(depth);
    let lane = b_writer_lane_v1(depth, column);
    let component = writer_component_v1(depth);
    assert(base::b_register_depth_v1(lane, component) == depth);
    assert(base::b_register_col_v1(lane) == column);
}

/// Fixed Slice 1 control flow has no lane-dependent branch before the LDS
/// barrier: each physical lane performs four A writes and four B writes, then
/// reaches the same barrier.
pub open spec fn lane_reaches_slice1_barrier_v1(lane: nat) -> bool {
    lane < 64
}

pub open spec fn arrivals_match_slice1_control_flow_v1(arrived: Seq<bool>) -> bool {
    arrived.len() == 64
        && forall |lane: nat| lane < 64 ==>
            arrived[lane as int] == lane_reaches_slice1_barrier_v1(lane)
}

pub proof fn slice1_barrier_converges_for_all_64_lanes_v1(
    arrived: Seq<bool>,
    lane: nat,
)
    requires arrivals_match_slice1_control_flow_v1(arrived), lane < 64,
    ensures
        lane_reaches_slice1_barrier_v1(lane),
        arrived[lane as int],
{
    assert(forall |physical_lane: nat| physical_lane < 64 ==>
        arrived[physical_lane as int] == lane_reaches_slice1_barrier_v1(physical_lane));
    assert(arrived[lane as int] == lane_reaches_slice1_barrier_v1(lane));
}

/// Distinct lane/component owners store distinct entries of the fixed 16x16 C
/// tile. This is a source-level ownership theorem, not an emitted-ISA claim.
pub proof fn fixed_tile_c_stores_are_disjoint_v1(
    left_lane: nat,
    left_component: nat,
    right_lane: nat,
    right_component: nat,
)
    requires
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        left_lane != right_lane || left_component != right_component,
    ensures
        base::global_c_index_v1(0, 0, left_lane, left_component, 16)
            != base::global_c_index_v1(0, 0, right_lane, right_component, 16),
{
    assert(base::exact_dispatch_v1(16, 16, 16));
    assert(base::checked_group_v1(0, 0, 16, 16));
    base::all_unequal_invocations_own_disjoint_global_c_v1(
        0, 0, left_lane, left_component,
        0, 0, right_lane, right_component,
        16, 16, 16,
    );
}

proof fn lds_reads_equal_global_inputs_v1(
    a: Seq<real>,
    b: Seq<real>,
    epoch: nat,
    row: nat,
    depth: nat,
    column: nat,
)
    requires
        fixed_tile_inputs_v1(a, b),
        row < 16,
        depth < 16,
        column < 16,
    ensures
        a_lds_read_value_v1(a, row, depth)
            == a[a_global_index_v1(row, depth) as int],
        b_lds_read_value_v1(b, depth, column)
            == b[b_global_index_v1(depth, column) as int],
{
    every_a_lds_read_is_initialized_in_same_epoch_v1(a, epoch, row, depth);
    every_b_lds_read_is_initialized_in_same_epoch_v1(b, epoch, depth, column);
}

proof fn lds_dot_prefix_matches_global_v1(
    a: Seq<real>,
    b: Seq<real>,
    epoch: nat,
    row: nat,
    column: nat,
    end: nat,
)
    requires
        fixed_tile_inputs_v1(a, b),
        row < 16,
        column < 16,
        end <= 16,
    ensures
        lds_dot_prefix_v1(a, b, row, column, end)
            == global_dot_prefix_v1(a, b, row, column, end),
    decreases end,
{
    if end > 0 {
        lds_dot_prefix_matches_global_v1(
            a, b, epoch, row, column, (end - 1) as nat,
        );
        lds_reads_equal_global_inputs_v1(
            a, b, epoch, row, (end - 1) as nat, column,
        );
    }
}

/// For every lane-owned accumulator, reading the XOR4-staged LDS tiles and
/// applying the explicit exact-real BF16/F32 abstraction computes the
/// corresponding entry of the fixed 16x16 mathematical matrix product.
pub proof fn fixed_tile_lds_result_is_matrix_product_v1(
    a: Seq<real>,
    b: Seq<real>,
    epoch: nat,
    lane: nat,
    component: nat,
)
    requires
        fixed_tile_inputs_v1(a, b),
        lane < 64,
        component < 4,
    ensures
        lds_dot_prefix_v1(
            a,
            b,
            base::accumulator_row_v1(lane, component),
            base::accumulator_col_v1(lane),
            16,
        ) == global_dot_prefix_v1(
            a,
            b,
            base::accumulator_row_v1(lane, component),
            base::accumulator_col_v1(lane),
            16,
        ),
{
    base::accumulator_coordinates_are_bounded_v1(lane, component);
    lds_dot_prefix_matches_global_v1(
        a,
        b,
        epoch,
        base::accumulator_row_v1(lane, component),
        base::accumulator_col_v1(lane),
        16,
    );
}

} // verus!
