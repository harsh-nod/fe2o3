use vstd::arithmetic::div_mod::{
    lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse, lemma_mod_bound,
};
use vstd::prelude::*;

verus! {

pub open spec fn tile_extent_v1() -> nat { 16 }
pub open spec fn tile_k_v1() -> nat { 16 }
pub open spec fn wave_lanes_v1() -> nat { 64 }
pub open spec fn components_per_lane_v1() -> nat { 4 }
pub open spec fn u32_max_v1() -> nat { 0xffff_ffff }
pub open spec fn u64_max_v1() -> nat { 0xffff_ffff_ffff_ffff }
pub open spec fn max_checked_bf16_elements_v1() -> nat { 0x7fff_ffff_ffff_ffff }
pub open spec fn max_checked_f32_elements_v1() -> nat { 0x3fff_ffff_ffff_ffff }

/// Source-level A-register formula pinned to AMD calculator commit 2ef91896.
pub open spec fn a_register_row_v1(lane: nat) -> nat {
    lane % 16
}

/// Source-level A-register formula pinned to AMD calculator commit 2ef91896.
pub open spec fn a_register_depth_v1(lane: nat, component: nat) -> nat {
    4 * (lane / 16) + component
}

/// Source-level B-register formula pinned to AMD calculator commit 2ef91896.
pub open spec fn b_register_depth_v1(lane: nat, component: nat) -> nat {
    4 * (lane / 16) + component
}

/// Source-level B-register formula pinned to AMD calculator commit 2ef91896.
pub open spec fn b_register_col_v1(lane: nat) -> nat {
    lane % 16
}

/// Source-level C/D-register formula pinned to AMD calculator commit 2ef91896.
pub open spec fn accumulator_row_v1(lane: nat, component: nat) -> nat {
    4 * (lane / 16) + component
}

/// Source-level C/D-register formula pinned to AMD calculator commit 2ef91896.
pub open spec fn accumulator_col_v1(lane: nat) -> nat {
    lane % 16
}

/// XOR for two values in `0..4`, written arithmetically for Verus.
pub open spec fn xor2_v1(left: nat, right: nat) -> nat
    recommends left < 4, right < 4,
{
    if left == 0 {
        right
    } else if left == 1 {
        if right == 0 { 1 } else if right == 1 { 0 } else if right == 2 { 3 } else { 2 }
    } else if left == 2 {
        if right == 0 { 2 } else if right == 1 { 3 } else if right == 2 { 0 } else { 1 }
    } else {
        if right == 0 { 3 } else if right == 1 { 2 } else if right == 2 { 1 } else { 0 }
    }
}

/// Physical XOR4 column for one bounded logical LDS coordinate.
pub open spec fn xor4_lds_col_v1(row: nat, col: nat) -> nat {
    xor2_v1(row % 4, col / 4) * 4 + col % 4
}

/// Physical XOR4 element index for one bounded logical LDS coordinate.
pub open spec fn xor4_lds_index_v1(row: nat, col: nat) -> nat {
    row * 16 + xor4_lds_col_v1(row, col)
}

/// A stages in logical `(row, depth)` order.
pub open spec fn a_lds_index_v1(lane: nat, component: nat) -> nat {
    xor4_lds_index_v1(
        a_register_row_v1(lane),
        a_register_depth_v1(lane, component),
    )
}

/// B stages transposed in logical `(column, depth)` order.
pub open spec fn b_transposed_lds_index_v1(lane: nat, component: nat) -> nat {
    xor4_lds_index_v1(
        b_register_col_v1(lane),
        b_register_depth_v1(lane, component),
    )
}

pub open spec fn checked_nonempty_shape_v1(m: nat, n: nat, k: nat) -> bool {
    &&& 0 < m <= u32_max_v1()
    &&& 0 < n <= u32_max_v1()
    &&& k <= u32_max_v1()
    &&& m * k <= max_checked_bf16_elements_v1()
    &&& k * n <= max_checked_bf16_elements_v1()
    &&& m * n <= max_checked_f32_elements_v1()
}

pub open spec fn exact_dispatch_v1(m: nat, n: nat, k: nat) -> bool {
    &&& checked_nonempty_shape_v1(m, n, k)
    &&& k > 0
    &&& m % 16 == 0
    &&& n % 16 == 0
    &&& k % 16 == 0
    &&& (n / 16) * 64 <= u32_max_v1()
}

pub open spec fn checked_group_v1(
    group_x: nat,
    group_y: nat,
    m: nat,
    n: nat,
) -> bool {
    group_x < n / 16 && group_y < m / 16
}

pub open spec fn global_c_row_v1(group_y: nat, lane: nat, component: nat) -> nat {
    group_y * 16 + accumulator_row_v1(lane, component)
}

pub open spec fn global_c_col_v1(group_x: nat, lane: nat) -> nat {
    group_x * 16 + accumulator_col_v1(lane)
}

pub open spec fn row_major_index_v1(row: nat, col: nat, columns: nat) -> nat {
    row * columns + col
}

pub open spec fn global_c_index_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    n: nat,
) -> nat {
    row_major_index_v1(
        global_c_row_v1(group_y, lane, component),
        global_c_col_v1(group_x, lane),
        n,
    )
}

pub open spec fn phase_depth_v1(phase: nat, offset: nat) -> nat {
    phase * 16 + offset
}

pub open spec fn a_global_row_v1(group_y: nat, lane: nat) -> nat {
    group_y * 16 + a_register_row_v1(lane)
}

pub open spec fn a_global_depth_v1(phase: nat, lane: nat, component: nat) -> nat {
    phase_depth_v1(phase, a_register_depth_v1(lane, component))
}

pub open spec fn b_global_depth_v1(phase: nat, lane: nat, component: nat) -> nat {
    phase_depth_v1(phase, b_register_depth_v1(lane, component))
}

pub open spec fn b_global_col_v1(group_x: nat, lane: nat) -> nat {
    group_x * 16 + b_register_col_v1(lane)
}

pub open spec fn empty_output_v1(m: nat, n: nat) -> bool {
    m == 0 || n == 0
}

pub open spec fn host_fill_positive_zero_v1(m: nat, n: nat, k: nat) -> bool {
    &&& checked_nonempty_shape_v1(m, n, k)
    &&& m % 16 == 0
    &&& n % 16 == 0
    &&& k == 0
}

pub open spec fn host_decision_operand_accesses_v1(m: nat, n: nat, k: nat) -> nat {
    if empty_output_v1(m, n) || host_fill_positive_zero_v1(m, n, k) {
        0
    } else {
        m * k + k * n
    }
}

proof fn div_mod_reconstructs_v1(value: nat, divisor: nat)
    requires divisor > 0,
    ensures
        value == (value / divisor) * divisor + value % divisor,
        value % divisor < divisor,
{
    lemma_mod_bound(value as int, divisor as int);
    lemma_fundamental_div_mod(value as int, divisor as int);
    assert(value == divisor * (value / divisor) + value % divisor);
    assert(divisor * (value / divisor) == (value / divisor) * divisor)
        by (nonlinear_arith);
}

proof fn bounded_quotient_v1(value: nat, blocks: nat, divisor: nat)
    requires
        divisor > 0,
        value < blocks * divisor,
    ensures
        value / divisor < blocks,
        value % divisor < divisor,
        value == (value / divisor) * divisor + value % divisor,
{
    div_mod_reconstructs_v1(value, divisor);
    if value / divisor >= blocks {
        assert((value / divisor) * divisor >= blocks * divisor)
            by (nonlinear_arith)
            requires
                value / divisor >= blocks,
                divisor > 0,
        ;
        assert(value >= blocks * divisor);
        assert(false);
    }
}

proof fn pack4_decomposes_v1(block: nat, inner: nat)
    requires inner < 4,
    ensures
        (block * 4 + inner) / 4 == block,
        (block * 4 + inner) % 4 == inner,
{
    lemma_fundamental_div_mod_converse(
        (block * 4 + inner) as int,
        4,
        block as int,
        inner as int,
    );
}

proof fn pack16_decomposes_v1(row: nat, col: nat)
    requires col < 16,
    ensures
        (row * 16 + col) / 16 == row,
        (row * 16 + col) % 16 == col,
{
    lemma_fundamental_div_mod_converse(
        (row * 16 + col) as int,
        16,
        row as int,
        col as int,
    );
}

proof fn packed_pair_is_injective_v1(
    left_block: nat,
    left_inner: nat,
    right_block: nat,
    right_inner: nat,
)
    requires
        left_inner < 4,
        right_inner < 4,
        left_block != right_block || left_inner != right_inner,
    ensures
        left_block * 4 + left_inner != right_block * 4 + right_inner,
{
    if left_block == right_block {
        assert(left_inner != right_inner);
    } else if left_block < right_block {
        assert(left_block * 4 + left_inner < (left_block + 1) * 4)
            by (nonlinear_arith)
            requires left_inner < 4,
        ;
        assert((left_block + 1) * 4 <= right_block * 4)
            by (nonlinear_arith)
            requires left_block + 1 <= right_block,
        ;
    } else {
        assert(right_block * 4 + right_inner < (right_block + 1) * 4)
            by (nonlinear_arith)
            requires right_inner < 4,
        ;
        assert((right_block + 1) * 4 <= left_block * 4)
            by (nonlinear_arith)
            requires right_block + 1 <= left_block,
        ;
    }
}

proof fn lane_coordinates_v1(lane: nat)
    requires lane < 64,
    ensures
        lane / 16 < 4,
        lane % 16 < 16,
        lane == (lane / 16) * 16 + lane % 16,
{
    bounded_quotient_v1(lane, 4, 16);
}

/// The official A register formula covers only `A[0..16][0..16]`.
pub proof fn a_register_coordinates_are_bounded_v1(lane: nat, component: nat)
    requires lane < 64, component < 4,
    ensures
        a_register_row_v1(lane) < 16,
        a_register_depth_v1(lane, component) < 16,
{
    lane_coordinates_v1(lane);
    assert(4 * (lane / 16) + component < 16) by (nonlinear_arith)
        requires
            lane / 16 < 4,
            component < 4,
    ;
}

/// The official B register formula covers only `B[0..16][0..16]`.
pub proof fn b_register_coordinates_are_bounded_v1(lane: nat, component: nat)
    requires lane < 64, component < 4,
    ensures
        b_register_depth_v1(lane, component) < 16,
        b_register_col_v1(lane) < 16,
{
    lane_coordinates_v1(lane);
    assert(4 * (lane / 16) + component < 16) by (nonlinear_arith)
        requires
            lane / 16 < 4,
            component < 4,
    ;
}

/// The official C/D formula covers only the 16x16 accumulator tile.
pub proof fn accumulator_coordinates_are_bounded_v1(lane: nat, component: nat)
    requires lane < 64, component < 4,
    ensures
        accumulator_row_v1(lane, component) < 16,
        accumulator_col_v1(lane) < 16,
{
    lane_coordinates_v1(lane);
    assert(4 * (lane / 16) + component < 16) by (nonlinear_arith)
        requires
            lane / 16 < 4,
            component < 4,
    ;
}

/// Every unequal lane/component pair has distinct A, B, and C/D coordinates.
pub proof fn lane_component_register_maps_are_injective_v1(
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
        a_register_row_v1(left_lane) != a_register_row_v1(right_lane)
            || a_register_depth_v1(left_lane, left_component)
                != a_register_depth_v1(right_lane, right_component),
        b_register_depth_v1(left_lane, left_component)
                != b_register_depth_v1(right_lane, right_component)
            || b_register_col_v1(left_lane) != b_register_col_v1(right_lane),
        accumulator_row_v1(left_lane, left_component)
                != accumulator_row_v1(right_lane, right_component)
            || accumulator_col_v1(left_lane) != accumulator_col_v1(right_lane),
{
    lane_coordinates_v1(left_lane);
    lane_coordinates_v1(right_lane);
    let left_block = left_lane / 16;
    let right_block = right_lane / 16;
    let left_remainder = left_lane % 16;
    let right_remainder = right_lane % 16;
    if left_remainder == right_remainder {
        if left_block == right_block {
            assert(left_lane == right_lane);
            assert(left_component != right_component);
        }
        packed_pair_is_injective_v1(
            left_block,
            left_component,
            right_block,
            right_component,
        );
    }
}

proof fn two_bit_xor_is_bounded_v1(left: nat, right: nat)
    requires left < 4, right < 4,
    ensures xor2_v1(left, right) < 4,
{
    assert(left == 0 || left == 1 || left == 2 || left == 3);
    assert(right == 0 || right == 1 || right == 2 || right == 3);
    if left == 0 {
    } else if left == 1 {
        if right == 0 {} else if right == 1 {} else if right == 2 {} else {}
    } else if left == 2 {
        if right == 0 {} else if right == 1 {} else if right == 2 {} else {}
    } else {
        if right == 0 {} else if right == 1 {} else if right == 2 {} else {}
    }
}

proof fn two_bit_xor_is_involutive_v1(left: nat, right: nat)
    requires left < 4, right < 4,
    ensures xor2_v1(left, xor2_v1(left, right)) == right,
{
    assert(left == 0 || left == 1 || left == 2 || left == 3);
    assert(right == 0 || right == 1 || right == 2 || right == 3);
    if left == 0 {
    } else if left == 1 {
        if right == 0 {} else if right == 1 {} else if right == 2 {} else {}
    } else if left == 2 {
        if right == 0 {} else if right == 1 {} else if right == 2 {} else {}
    } else {
        if right == 0 {} else if right == 1 {} else if right == 2 {} else {}
    }
}

/// XOR4 maps every logical 16x16 coordinate into physical `0..256`.
pub proof fn xor4_physical_index_is_bounded_v1(row: nat, col: nat)
    requires row < 16, col < 16,
    ensures
        xor4_lds_col_v1(row, col) < 16,
        xor4_lds_index_v1(row, col) < 256,
{
    bounded_quotient_v1(row, 4, 4);
    bounded_quotient_v1(col, 4, 4);
    two_bit_xor_is_bounded_v1(row % 4, col / 4);
    assert(xor2_v1(row % 4, col / 4) * 4 + col % 4 < 16)
        by (nonlinear_arith)
        requires
            xor2_v1(row % 4, col / 4) < 4,
            col % 4 < 4,
    ;
    assert(row * 16 + xor4_lds_col_v1(row, col) < 256)
        by (nonlinear_arith)
        requires
            row < 16,
            xor4_lds_col_v1(row, col) < 16,
    ;
}

/// XOR4 is its own physical-column inverse for a fixed logical row.
pub proof fn xor4_column_round_trips_v1(row: nat, col: nat)
    requires row < 16, col < 16,
    ensures xor4_lds_col_v1(row, xor4_lds_col_v1(row, col)) == col,
{
    bounded_quotient_v1(row, 4, 4);
    bounded_quotient_v1(col, 4, 4);
    let row_xor = row % 4;
    let block = col / 4;
    let inner = col % 4;
    let physical_block = xor2_v1(row_xor, block);
    two_bit_xor_is_bounded_v1(row_xor, block);
    pack4_decomposes_v1(physical_block, inner);
    two_bit_xor_is_involutive_v1(row_xor, block);
    assert(col == block * 4 + inner);
}

/// The physical-index inverse reconstructs every logical XOR4 coordinate.
pub proof fn xor4_logical_coordinate_round_trips_v1(row: nat, col: nat)
    requires row < 16, col < 16,
    ensures
        xor4_lds_index_v1(row, col) / 16 == row,
        xor4_lds_col_v1(
            xor4_lds_index_v1(row, col) / 16,
            xor4_lds_index_v1(row, col) % 16,
        ) == col,
{
    xor4_physical_index_is_bounded_v1(row, col);
    pack16_decomposes_v1(row, xor4_lds_col_v1(row, col));
    xor4_column_round_trips_v1(row, col);
}

/// Distinct logical coordinates never alias one XOR4 physical index.
pub proof fn xor4_physical_index_is_injective_v1(
    left_row: nat,
    left_col: nat,
    right_row: nat,
    right_col: nat,
)
    requires
        left_row < 16,
        left_col < 16,
        right_row < 16,
        right_col < 16,
        left_row != right_row || left_col != right_col,
    ensures
        xor4_lds_index_v1(left_row, left_col)
            != xor4_lds_index_v1(right_row, right_col),
{
    xor4_logical_coordinate_round_trips_v1(left_row, left_col);
    xor4_logical_coordinate_round_trips_v1(right_row, right_col);
    if xor4_lds_index_v1(left_row, left_col)
        == xor4_lds_index_v1(right_row, right_col)
    {
        assert(left_row == right_row);
        assert(left_col == right_col);
        assert(false);
    }
}

/// Every physical slot in `0..256` has one logical XOR4 preimage.
pub proof fn xor4_physical_layout_is_permutation_v1(index: nat)
    requires index < 256,
    ensures
        index / 16 < 16,
        xor4_lds_col_v1(index / 16, index % 16) < 16,
        xor4_lds_index_v1(
            index / 16,
            xor4_lds_col_v1(index / 16, index % 16),
        ) == index,
{
    bounded_quotient_v1(index, 16, 16);
    let row = index / 16;
    let physical_col = index % 16;
    xor4_physical_index_is_bounded_v1(row, physical_col);
    xor4_column_round_trips_v1(row, physical_col);
    assert(index == row * 16 + physical_col);
}

/// A and transposed-B staging both stay inside their separate 256-slot tiles.
pub proof fn a_and_b_staging_are_bounded_v1(lane: nat, component: nat)
    requires lane < 64, component < 4,
    ensures
        a_lds_index_v1(lane, component) < 256,
        b_transposed_lds_index_v1(lane, component) < 256,
{
    a_register_coordinates_are_bounded_v1(lane, component);
    b_register_coordinates_are_bounded_v1(lane, component);
    xor4_physical_index_is_bounded_v1(
        a_register_row_v1(lane),
        a_register_depth_v1(lane, component),
    );
    xor4_physical_index_is_bounded_v1(
        b_register_col_v1(lane),
        b_register_depth_v1(lane, component),
    );
}

/// Unequal lane/components never alias within the A staging tile.
pub proof fn distinct_lane_components_have_disjoint_a_lds_v1(
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
        a_lds_index_v1(left_lane, left_component)
            != a_lds_index_v1(right_lane, right_component),
{
    a_register_coordinates_are_bounded_v1(left_lane, left_component);
    a_register_coordinates_are_bounded_v1(right_lane, right_component);
    lane_component_register_maps_are_injective_v1(
        left_lane, left_component, right_lane, right_component,
    );
    xor4_physical_index_is_injective_v1(
        a_register_row_v1(left_lane),
        a_register_depth_v1(left_lane, left_component),
        a_register_row_v1(right_lane),
        a_register_depth_v1(right_lane, right_component),
    );
}

/// Unequal lane/components never alias within the transposed-B staging tile.
pub proof fn distinct_lane_components_have_disjoint_b_lds_v1(
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
        b_transposed_lds_index_v1(left_lane, left_component)
            != b_transposed_lds_index_v1(right_lane, right_component),
{
    b_register_coordinates_are_bounded_v1(left_lane, left_component);
    b_register_coordinates_are_bounded_v1(right_lane, right_component);
    lane_component_register_maps_are_injective_v1(
        left_lane, left_component, right_lane, right_component,
    );
    xor4_physical_index_is_injective_v1(
        b_register_col_v1(left_lane),
        b_register_depth_v1(left_lane, left_component),
        b_register_col_v1(right_lane),
        b_register_depth_v1(right_lane, right_component),
    );
}

proof fn exact_multiple_reconstructs_v1(value: nat)
    requires value % 16 == 0,
    ensures value == (value / 16) * 16,
{
    div_mod_reconstructs_v1(value, 16);
}

/// Every checked group owns one complete 16x16 output tile in M/N bounds.
pub proof fn checked_workgroup_origin_stays_in_bounds_v1(
    group_x: nat,
    group_y: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        exact_dispatch_v1(m, n, k),
        checked_group_v1(group_x, group_y, m, n),
    ensures
        group_y * 16 + 16 <= m,
        group_x * 16 + 16 <= n,
{
    exact_multiple_reconstructs_v1(m);
    exact_multiple_reconstructs_v1(n);
    assert(group_y + 1 <= m / 16);
    assert((group_y + 1) * 16 <= (m / 16) * 16) by (nonlinear_arith)
        requires group_y + 1 <= m / 16,
    ;
    assert(group_x + 1 <= n / 16);
    assert((group_x + 1) * 16 <= (n / 16) * 16) by (nonlinear_arith)
        requires group_x + 1 <= n / 16,
    ;
}

proof fn row_major_coordinates_are_injective_v1(
    left_row: nat,
    left_col: nat,
    right_row: nat,
    right_col: nat,
    columns: nat,
)
    requires
        left_col < columns,
        right_col < columns,
        left_row != right_row || left_col != right_col,
    ensures
        row_major_index_v1(left_row, left_col, columns)
            != row_major_index_v1(right_row, right_col, columns),
{
    assert(columns > 0);
    if left_row == right_row {
        assert(left_col != right_col);
    } else if left_row < right_row {
        assert(left_row * columns + left_col < (left_row + 1) * columns)
            by (nonlinear_arith)
            requires left_col < columns, columns > 0,
        ;
        assert((left_row + 1) * columns <= right_row * columns)
            by (nonlinear_arith)
            requires left_row + 1 <= right_row, columns > 0,
        ;
    } else {
        assert(right_row * columns + right_col < (right_row + 1) * columns)
            by (nonlinear_arith)
            requires right_col < columns, columns > 0,
        ;
        assert((right_row + 1) * columns <= left_row * columns)
            by (nonlinear_arith)
            requires right_row + 1 <= left_row, columns > 0,
        ;
    }
}

/// Row-major itself is injective for bounded logical 16x16 coordinates.
pub proof fn distinct_logical_coordinates_have_distinct_row_major_v1(
    left_row: nat,
    left_col: nat,
    right_row: nat,
    right_col: nat,
)
    requires
        left_row < 16,
        left_col < 16,
        right_row < 16,
        right_col < 16,
        left_row != right_row || left_col != right_col,
    ensures
        left_row * 16 + left_col != right_row * 16 + right_col,
{
    row_major_coordinates_are_injective_v1(
        left_row, left_col, right_row, right_col, 16,
    );
}

/// Every valid group/lane/component maps to one in-bounds global C element.
pub proof fn checked_accumulator_output_is_in_bounds_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        exact_dispatch_v1(m, n, k),
        checked_group_v1(group_x, group_y, m, n),
        lane < 64,
        component < 4,
    ensures
        global_c_row_v1(group_y, lane, component) < m,
        global_c_col_v1(group_x, lane) < n,
        global_c_index_v1(group_x, group_y, lane, component, n) < m * n,
{
    checked_workgroup_origin_stays_in_bounds_v1(group_x, group_y, m, n, k);
    accumulator_coordinates_are_bounded_v1(lane, component);
    assert(global_c_row_v1(group_y, lane, component) + 1 <= m);
    assert((global_c_row_v1(group_y, lane, component) + 1) * n <= m * n)
        by (nonlinear_arith)
        requires
            global_c_row_v1(group_y, lane, component) + 1 <= m,
            n > 0,
    ;
    assert(global_c_row_v1(group_y, lane, component) * n
        + global_c_col_v1(group_x, lane)
        < (global_c_row_v1(group_y, lane, component) + 1) * n)
        by (nonlinear_arith)
        requires
            global_c_col_v1(group_x, lane) < n,
            n > 0,
    ;
}

/// All unequal `(group_x, group_y, lane, component)` tuples own disjoint C.
pub proof fn all_unequal_invocations_own_disjoint_global_c_v1(
    left_group_x: nat,
    left_group_y: nat,
    left_lane: nat,
    left_component: nat,
    right_group_x: nat,
    right_group_y: nat,
    right_lane: nat,
    right_component: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        exact_dispatch_v1(m, n, k),
        checked_group_v1(left_group_x, left_group_y, m, n),
        checked_group_v1(right_group_x, right_group_y, m, n),
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        left_group_x != right_group_x
            || left_group_y != right_group_y
            || left_lane != right_lane
            || left_component != right_component,
    ensures
        global_c_index_v1(
            left_group_x, left_group_y, left_lane, left_component, n,
        ) != global_c_index_v1(
            right_group_x, right_group_y, right_lane, right_component, n,
        ),
{
    checked_accumulator_output_is_in_bounds_v1(
        left_group_x, left_group_y, left_lane, left_component, m, n, k,
    );
    checked_accumulator_output_is_in_bounds_v1(
        right_group_x, right_group_y, right_lane, right_component, m, n, k,
    );

    let left_row = global_c_row_v1(left_group_y, left_lane, left_component);
    let left_col = global_c_col_v1(left_group_x, left_lane);
    let right_row = global_c_row_v1(right_group_y, right_lane, right_component);
    let right_col = global_c_col_v1(right_group_x, right_lane);

    if left_group_y != right_group_y {
        if left_group_y < right_group_y {
            assert(left_group_y * 16 + 16 <= right_group_y * 16)
                by (nonlinear_arith)
                requires left_group_y + 1 <= right_group_y,
            ;
            assert(left_row < right_row);
        } else {
            assert(right_group_y * 16 + 16 <= left_group_y * 16)
                by (nonlinear_arith)
                requires right_group_y + 1 <= left_group_y,
            ;
            assert(right_row < left_row);
        }
    } else if left_group_x != right_group_x {
        if left_group_x < right_group_x {
            assert(left_group_x * 16 + 16 <= right_group_x * 16)
                by (nonlinear_arith)
                requires left_group_x + 1 <= right_group_x,
            ;
            assert(left_col < right_col);
        } else {
            assert(right_group_x * 16 + 16 <= left_group_x * 16)
                by (nonlinear_arith)
                requires right_group_x + 1 <= left_group_x,
            ;
            assert(right_col < left_col);
        }
    } else {
        assert(left_lane != right_lane || left_component != right_component);
        lane_component_register_maps_are_injective_v1(
            left_lane, left_component, right_lane, right_component,
        );
        assert(left_row != right_row || left_col != right_col);
    }
    row_major_coordinates_are_injective_v1(
        left_row, left_col, right_row, right_col, n,
    );
}

/// Every official A load for each admitted K phase is in bounds.
pub proof fn a_phase_load_is_in_bounds_v1(
    group_x: nat,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        exact_dispatch_v1(m, n, k),
        checked_group_v1(group_x, group_y, m, n),
        phase < k / 16,
        lane < 64,
        component < 4,
    ensures
        a_global_row_v1(group_y, lane) < m,
        a_global_depth_v1(phase, lane, component) < k,
        row_major_index_v1(
            a_global_row_v1(group_y, lane),
            a_global_depth_v1(phase, lane, component),
            k,
        ) < m * k,
{
    checked_workgroup_origin_stays_in_bounds_v1(group_x, group_y, m, n, k);
    a_register_coordinates_are_bounded_v1(lane, component);
    exact_multiple_reconstructs_v1(k);
    assert(phase + 1 <= k / 16);
    assert((phase + 1) * 16 <= k) by (nonlinear_arith)
        requires
            phase + 1 <= k / 16,
            k == (k / 16) * 16,
    ;
    assert(a_global_depth_v1(phase, lane, component) < (phase + 1) * 16)
        by (nonlinear_arith)
        requires a_register_depth_v1(lane, component) < 16,
    ;
    assert(a_global_row_v1(group_y, lane) + 1 <= m);
    assert((a_global_row_v1(group_y, lane) + 1) * k <= m * k)
        by (nonlinear_arith)
        requires
            a_global_row_v1(group_y, lane) + 1 <= m,
            k > 0,
    ;
    assert(a_global_row_v1(group_y, lane) * k
        + a_global_depth_v1(phase, lane, component)
        < (a_global_row_v1(group_y, lane) + 1) * k)
        by (nonlinear_arith)
        requires
            a_global_depth_v1(phase, lane, component) < k,
            k > 0,
    ;
}

/// Every official B load for each admitted K phase is in bounds.
pub proof fn b_phase_load_is_in_bounds_v1(
    group_x: nat,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        exact_dispatch_v1(m, n, k),
        checked_group_v1(group_x, group_y, m, n),
        phase < k / 16,
        lane < 64,
        component < 4,
    ensures
        b_global_depth_v1(phase, lane, component) < k,
        b_global_col_v1(group_x, lane) < n,
        row_major_index_v1(
            b_global_depth_v1(phase, lane, component),
            b_global_col_v1(group_x, lane),
            n,
        ) < k * n,
{
    checked_workgroup_origin_stays_in_bounds_v1(group_x, group_y, m, n, k);
    b_register_coordinates_are_bounded_v1(lane, component);
    exact_multiple_reconstructs_v1(k);
    assert(phase + 1 <= k / 16);
    assert((phase + 1) * 16 <= k) by (nonlinear_arith)
        requires
            phase + 1 <= k / 16,
            k == (k / 16) * 16,
    ;
    assert(b_global_depth_v1(phase, lane, component) < (phase + 1) * 16)
        by (nonlinear_arith)
        requires b_register_depth_v1(lane, component) < 16,
    ;
    assert(b_global_depth_v1(phase, lane, component) + 1 <= k);
    assert((b_global_depth_v1(phase, lane, component) + 1) * n <= k * n)
        by (nonlinear_arith)
        requires
            b_global_depth_v1(phase, lane, component) + 1 <= k,
            n > 0,
    ;
    assert(b_global_depth_v1(phase, lane, component) * n
        + b_global_col_v1(group_x, lane)
        < (b_global_depth_v1(phase, lane, component) + 1) * n)
        by (nonlinear_arith)
        requires
            b_global_col_v1(group_x, lane) < n,
            n > 0,
    ;
}

/// Exact K=16 phases cover every depth in `0..K` exactly once.
pub proof fn k_phases_partition_every_depth_v1(depth: nat, k: nat)
    requires k > 0, k % 16 == 0, depth < k,
    ensures
        depth / 16 < k / 16,
        depth % 16 < 16,
        phase_depth_v1(depth / 16, depth % 16) == depth,
{
    bounded_quotient_v1(depth, k / 16, 16);
    exact_multiple_reconstructs_v1(k);
}

/// Different phase/offset pairs never name the same K depth.
pub proof fn distinct_k_phase_offsets_are_disjoint_v1(
    left_phase: nat,
    left_offset: nat,
    right_phase: nat,
    right_offset: nat,
    k: nat,
)
    requires
        k > 0,
        k % 16 == 0,
        left_phase < k / 16,
        right_phase < k / 16,
        left_offset < 16,
        right_offset < 16,
        left_phase != right_phase || left_offset != right_offset,
    ensures
        phase_depth_v1(left_phase, left_offset)
            != phase_depth_v1(right_phase, right_offset),
{
    if left_phase == right_phase {
        assert(left_offset != right_offset);
    } else if left_phase < right_phase {
        assert(left_phase * 16 + left_offset < (left_phase + 1) * 16)
            by (nonlinear_arith)
            requires left_offset < 16,
        ;
        assert((left_phase + 1) * 16 <= right_phase * 16)
            by (nonlinear_arith)
            requires left_phase + 1 <= right_phase,
        ;
    } else {
        assert(right_phase * 16 + right_offset < (right_phase + 1) * 16)
            by (nonlinear_arith)
            requires right_offset < 16,
        ;
        assert((right_phase + 1) * 16 <= left_phase * 16)
            by (nonlinear_arith)
            requires right_phase + 1 <= left_phase,
        ;
    }
}

proof fn checked_element_address_fits_u64_v1(
    elements: nat,
    index: nat,
    element_bytes: nat,
)
    requires
        index < elements,
        (element_bytes == 2 && elements <= max_checked_bf16_elements_v1())
            || (element_bytes == 4 && elements <= max_checked_f32_elements_v1()),
    ensures index * element_bytes + element_bytes <= u64_max_v1(),
{
    assert(index + 1 <= elements);
    assert((index + 1) * element_bytes <= elements * element_bytes)
        by (nonlinear_arith)
        requires
            index + 1 <= elements,
            element_bytes > 0,
    ;
    if element_bytes == 2 {
        assert(elements * 2 <= max_checked_bf16_elements_v1() * 2)
            by (nonlinear_arith)
            requires elements <= max_checked_bf16_elements_v1(),
        ;
        assert(max_checked_bf16_elements_v1() * 2 <= u64_max_v1());
    } else {
        assert(element_bytes == 4);
        assert(elements * 4 <= max_checked_f32_elements_v1() * 4)
            by (nonlinear_arith)
            requires elements <= max_checked_f32_elements_v1(),
        ;
        assert(max_checked_f32_elements_v1() * 4 <= u64_max_v1());
    }
    assert(index * element_bytes + element_bytes == (index + 1) * element_bytes)
        by (nonlinear_arith);
}

/// Exact A/B/C byte-end arithmetic fits the host contract's u64 bounds.
pub proof fn checked_matrix_addresses_fit_u64_v1(
    group_x: nat,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        exact_dispatch_v1(m, n, k),
        checked_group_v1(group_x, group_y, m, n),
        phase < k / 16,
        lane < 64,
        component < 4,
    ensures
        row_major_index_v1(
            a_global_row_v1(group_y, lane),
            a_global_depth_v1(phase, lane, component),
            k,
        ) * 2 + 2 <= u64_max_v1(),
        row_major_index_v1(
            b_global_depth_v1(phase, lane, component),
            b_global_col_v1(group_x, lane),
            n,
        ) * 2 + 2 <= u64_max_v1(),
        global_c_index_v1(group_x, group_y, lane, component, n) * 4 + 4
            <= u64_max_v1(),
{
    a_phase_load_is_in_bounds_v1(
        group_x, group_y, phase, lane, component, m, n, k,
    );
    b_phase_load_is_in_bounds_v1(
        group_x, group_y, phase, lane, component, m, n, k,
    );
    checked_accumulator_output_is_in_bounds_v1(
        group_x, group_y, lane, component, m, n, k,
    );
    checked_element_address_fits_u64_v1(
        m * k,
        row_major_index_v1(
            a_global_row_v1(group_y, lane),
            a_global_depth_v1(phase, lane, component),
            k,
        ),
        2,
    );
    checked_element_address_fits_u64_v1(
        k * n,
        row_major_index_v1(
            b_global_depth_v1(phase, lane, component),
            b_global_col_v1(group_x, lane),
            n,
        ),
        2,
    );
    checked_element_address_fits_u64_v1(
        m * n,
        global_c_index_v1(group_x, group_y, lane, component, n),
        4,
    );
}

/// Empty output performs no A or B storage access.
pub proof fn empty_output_no_dispatch_reads_no_operands_v1(m: nat, n: nat, k: nat)
    requires
        m <= u32_max_v1(),
        n <= u32_max_v1(),
        k <= u32_max_v1(),
        empty_output_v1(m, n),
    ensures host_decision_operand_accesses_v1(m, n, k) == 0,
{
}

/// Nonempty K=0 host fill performs no A or B storage access.
pub proof fn zero_k_host_fill_reads_no_operands_v1(m: nat, n: nat, k: nat)
    requires host_fill_positive_zero_v1(m, n, k),
    ensures
        host_decision_operand_accesses_v1(m, n, k) == 0,
        m * k == 0,
        k * n == 0,
{
}

} // verus!
