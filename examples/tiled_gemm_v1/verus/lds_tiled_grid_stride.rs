use vstd::arithmetic::div_mod::{
    lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse,
};
use vstd::prelude::*;

#[path = "tiled_gemm_host_contract.rs"]
mod base;

verus! {

/// Slice 3 is a bounded source-level model for a grid of independent K=16
/// tiles. No kernel-source correspondence or backend refinement is established.
/// No emitted-machine-code safety or hardware behavior is established.
pub open spec fn grid_k_v1() -> nat { 16 }
pub open spec fn grid_tile_extent_v1() -> nat { 16 }
pub open spec fn grid_wave_lanes_v1() -> nat { 64 }
pub open spec fn grid_components_per_lane_v1() -> nat { 4 }
pub open spec fn grid_u32_max_v1() -> nat { 0xffff_ffff }
pub open spec fn grid_u64_max_v1() -> nat { 0xffff_ffff_ffff_ffff }

pub open spec fn grid_tiles_x_v1(n: nat) -> nat { n / grid_tile_extent_v1() }
pub open spec fn grid_tiles_y_v1(m: nat) -> nat { m / grid_tile_extent_v1() }
pub open spec fn grid_threads_x_v1(n: nat) -> nat {
    grid_tiles_x_v1(n) * grid_wave_lanes_v1()
}
pub open spec fn grid_workgroup_count_v1(m: nat, n: nat) -> nat {
    grid_tiles_x_v1(n) * grid_tiles_y_v1(m)
}

pub open spec fn strided_footprint_v1(
    rows: nat,
    logical_columns: nat,
    stride: nat,
) -> nat {
    if rows == 0 { 0 } else { (rows - 1) as nat * stride + logical_columns }
}

/// M/N and row strides are launcher-visible u32 values. Allocation lengths,
/// strided element indices, and byte endpoints are checked in u64 arithmetic.
/// K is exactly 16; tails and multiple K phases are outside this milestone.
pub open spec fn checked_grid_problem_v1(
    m: nat,
    n: nat,
    lda: nat,
    ldb: nat,
    ldc: nat,
    a_len: nat,
    b_len: nat,
    c_len: nat,
) -> bool {
    &&& 0 < m <= grid_u32_max_v1()
    &&& 0 < n <= grid_u32_max_v1()
    &&& m % grid_tile_extent_v1() == 0
    &&& n % grid_tile_extent_v1() == 0
    &&& grid_k_v1() <= lda <= grid_u32_max_v1()
    &&& n <= ldb <= grid_u32_max_v1()
    &&& n <= ldc <= grid_u32_max_v1()
    &&& grid_threads_x_v1(n) <= grid_u32_max_v1()
    &&& grid_tiles_y_v1(m) <= grid_u32_max_v1()
    &&& grid_workgroup_count_v1(m, n) <= grid_u32_max_v1()
    &&& strided_footprint_v1(m, grid_k_v1(), lda) <= a_len
    &&& strided_footprint_v1(grid_k_v1(), n, ldb) <= b_len
    &&& strided_footprint_v1(m, n, ldc) <= c_len
    &&& a_len * 2 <= grid_u64_max_v1()
    &&& b_len * 2 <= grid_u64_max_v1()
    &&& c_len * 4 <= grid_u64_max_v1()
}

pub open spec fn checked_grid_workgroup_v1(
    group_x: nat,
    group_y: nat,
    m: nat,
    n: nat,
) -> bool {
    group_x < grid_tiles_x_v1(n) && group_y < grid_tiles_y_v1(m)
}

pub open spec fn grid_tile_row_v1(group_y: nat) -> nat { group_y }
pub open spec fn grid_tile_col_v1(group_x: nat) -> nat { group_x }
pub open spec fn grid_tile_origin_row_v1(group_y: nat) -> nat {
    grid_tile_row_v1(group_y) * grid_tile_extent_v1()
}
pub open spec fn grid_tile_origin_col_v1(group_x: nat) -> nat {
    grid_tile_col_v1(group_x) * grid_tile_extent_v1()
}
pub open spec fn grid_tile_linear_v1(group_x: nat, group_y: nat, n: nat) -> nat {
    grid_tile_row_v1(group_y) * grid_tiles_x_v1(n) + grid_tile_col_v1(group_x)
}

pub open spec fn grid_a_row_v1(group_y: nat, lane: nat) -> nat {
    grid_tile_origin_row_v1(group_y) + base::a_register_row_v1(lane)
}
pub open spec fn grid_a_depth_v1(lane: nat, component: nat) -> nat {
    base::a_register_depth_v1(lane, component)
}
pub open spec fn grid_a_index_v1(
    group_y: nat,
    lane: nat,
    component: nat,
    lda: nat,
) -> nat {
    grid_a_row_v1(group_y, lane) * lda + grid_a_depth_v1(lane, component)
}

pub open spec fn grid_b_depth_v1(lane: nat, component: nat) -> nat {
    base::b_register_depth_v1(lane, component)
}
pub open spec fn grid_b_col_v1(group_x: nat, lane: nat) -> nat {
    grid_tile_origin_col_v1(group_x) + base::b_register_col_v1(lane)
}
pub open spec fn grid_b_index_v1(
    group_x: nat,
    lane: nat,
    component: nat,
    ldb: nat,
) -> nat {
    grid_b_depth_v1(lane, component) * ldb + grid_b_col_v1(group_x, lane)
}

pub open spec fn grid_c_row_v1(
    group_y: nat,
    lane: nat,
    component: nat,
) -> nat {
    grid_tile_origin_row_v1(group_y) + base::accumulator_row_v1(lane, component)
}
pub open spec fn grid_c_col_v1(group_x: nat, lane: nat) -> nat {
    grid_tile_origin_col_v1(group_x) + base::accumulator_col_v1(lane)
}
pub open spec fn grid_c_index_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    ldc: nat,
) -> nat {
    grid_c_row_v1(group_y, lane, component) * ldc
        + grid_c_col_v1(group_x, lane)
}

proof fn exact_multiple_reconstructs_grid_v1(value: nat)
    requires value % 16 == 0,
    ensures value == (value / 16) * 16,
{
    lemma_fundamental_div_mod(value as int, 16);
    assert(value == 16 * (value / 16) + value % 16);
    assert(16 * (value / 16) == (value / 16) * 16)
        by (nonlinear_arith);
}

proof fn packed_grid_coordinate_v1(block: nat, inner: nat)
    requires inner < 16,
    ensures
        (block * 16 + inner) / 16 == block,
        (block * 16 + inner) % 16 == inner,
{
    lemma_fundamental_div_mod_converse(
        (block * 16 + inner) as int,
        16,
        block as int,
        inner as int,
    );
}

proof fn strided_index_is_in_footprint_v1(
    row: nat,
    column: nat,
    rows: nat,
    logical_columns: nat,
    stride: nat,
)
    requires
        rows > 0,
        row < rows,
        column < logical_columns,
        logical_columns <= stride,
    ensures
        row * stride + column
            < strided_footprint_v1(rows, logical_columns, stride),
{
    if row + 1 < rows {
        assert(row * stride + column < (row + 1) * stride)
            by (nonlinear_arith)
            requires column < logical_columns, logical_columns <= stride;
        assert((row + 1) * stride <= (rows - 1) as nat * stride)
            by (nonlinear_arith)
            requires row + 1 <= (rows - 1) as nat;
    } else {
        assert(row + 1 == rows);
        assert(row == (rows - 1) as nat);
    }
}

proof fn strided_coordinates_are_injective_v1(
    left_row: nat,
    left_col: nat,
    right_row: nat,
    right_col: nat,
    stride: nat,
)
    requires
        left_col < stride,
        right_col < stride,
        left_row != right_row || left_col != right_col,
    ensures
        left_row * stride + left_col != right_row * stride + right_col,
{
    assert(stride > 0);
    if left_row == right_row {
        assert(left_col != right_col);
    } else if left_row < right_row {
        assert(left_row * stride + left_col < (left_row + 1) * stride)
            by (nonlinear_arith)
            requires left_col < stride, stride > 0;
        assert((left_row + 1) * stride <= right_row * stride)
            by (nonlinear_arith)
            requires left_row + 1 <= right_row, stride > 0;
    } else {
        assert(right_row * stride + right_col < (right_row + 1) * stride)
            by (nonlinear_arith)
            requires right_col < stride, stride > 0;
        assert((right_row + 1) * stride <= left_row * stride)
            by (nonlinear_arith)
            requires right_row + 1 <= left_row, stride > 0;
    }
}

/// Checked launcher geometry is nonempty, reconstructs M/N exactly, and fits
/// the admitted u32 thread-axis and bounded total-workgroup arithmetic.
pub proof fn checked_grid_derivation_is_exact_v1(
    m: nat,
    n: nat,
    lda: nat,
    ldb: nat,
    ldc: nat,
    a_len: nat,
    b_len: nat,
    c_len: nat,
)
    requires checked_grid_problem_v1(m, n, lda, ldb, ldc, a_len, b_len, c_len),
    ensures
        0 < grid_tiles_x_v1(n),
        0 < grid_tiles_y_v1(m),
        n == grid_tiles_x_v1(n) * 16,
        m == grid_tiles_y_v1(m) * 16,
        grid_threads_x_v1(n) <= grid_u32_max_v1(),
        grid_tiles_y_v1(m) <= grid_u32_max_v1(),
        grid_workgroup_count_v1(m, n) <= grid_u32_max_v1(),
{
    exact_multiple_reconstructs_grid_v1(m);
    exact_multiple_reconstructs_grid_v1(n);
    if grid_tiles_x_v1(n) == 0 {
        assert(n == 0);
        assert(false);
    }
    if grid_tiles_y_v1(m) == 0 {
        assert(m == 0);
        assert(false);
    }
}

proof fn checked_grid_tile_origins_v1(
    group_x: nat,
    group_y: nat,
    m: nat,
    n: nat,
)
    requires
        m > 0,
        n > 0,
        m % 16 == 0,
        n % 16 == 0,
        checked_grid_workgroup_v1(group_x, group_y, m, n),
    ensures
        grid_tile_origin_row_v1(group_y) + 16 <= m,
        grid_tile_origin_col_v1(group_x) + 16 <= n,
{
    exact_multiple_reconstructs_grid_v1(m);
    exact_multiple_reconstructs_grid_v1(n);
    assert(group_y + 1 <= grid_tiles_y_v1(m));
    assert((group_y + 1) * 16 <= grid_tiles_y_v1(m) * 16)
        by (nonlinear_arith)
        requires group_y + 1 <= grid_tiles_y_v1(m);
    assert(group_x + 1 <= grid_tiles_x_v1(n));
    assert((group_x + 1) * 16 <= grid_tiles_x_v1(n) * 16)
        by (nonlinear_arith)
        requires group_x + 1 <= grid_tiles_x_v1(n);
}

/// Different checked workgroups map to different tile coordinates and tile
/// linear IDs; no scheduler ordering or cross-workgroup synchronization is
/// assumed.
pub proof fn workgroup_to_tile_mapping_is_injective_v1(
    left_group_x: nat,
    left_group_y: nat,
    right_group_x: nat,
    right_group_y: nat,
    m: nat,
    n: nat,
)
    requires
        m > 0,
        n > 0,
        m % 16 == 0,
        n % 16 == 0,
        checked_grid_workgroup_v1(left_group_x, left_group_y, m, n),
        checked_grid_workgroup_v1(right_group_x, right_group_y, m, n),
        left_group_x != right_group_x || left_group_y != right_group_y,
    ensures
        grid_tile_row_v1(left_group_y) != grid_tile_row_v1(right_group_y)
            || grid_tile_col_v1(left_group_x) != grid_tile_col_v1(right_group_x),
        grid_tile_linear_v1(left_group_x, left_group_y, n)
            != grid_tile_linear_v1(right_group_x, right_group_y, n),
{
    strided_coordinates_are_injective_v1(
        left_group_y,
        left_group_x,
        right_group_y,
        right_group_x,
        grid_tiles_x_v1(n),
    );
}

/// Every one of the four cooperative A and B loads issued by a checked lane
/// is inside the corresponding allocation and has a u64-safe byte endpoint.
pub proof fn all_grid_global_a_b_loads_are_in_bounds_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    lda: nat,
    ldb: nat,
    ldc: nat,
    a_len: nat,
    b_len: nat,
    c_len: nat,
)
    requires
        checked_grid_problem_v1(m, n, lda, ldb, ldc, a_len, b_len, c_len),
        checked_grid_workgroup_v1(group_x, group_y, m, n),
        lane < grid_wave_lanes_v1(),
        component < grid_components_per_lane_v1(),
    ensures
        grid_a_row_v1(group_y, lane) < m,
        grid_a_depth_v1(lane, component) < grid_k_v1(),
        grid_a_index_v1(group_y, lane, component, lda) < a_len,
        (grid_a_index_v1(group_y, lane, component, lda) + 1) * 2
            <= grid_u64_max_v1(),
        grid_b_depth_v1(lane, component) < grid_k_v1(),
        grid_b_col_v1(group_x, lane) < n,
        grid_b_index_v1(group_x, lane, component, ldb) < b_len,
        (grid_b_index_v1(group_x, lane, component, ldb) + 1) * 2
            <= grid_u64_max_v1(),
{
    checked_grid_tile_origins_v1(group_x, group_y, m, n);
    base::a_register_coordinates_are_bounded_v1(lane, component);
    base::b_register_coordinates_are_bounded_v1(lane, component);
    strided_index_is_in_footprint_v1(
        grid_a_row_v1(group_y, lane),
        grid_a_depth_v1(lane, component),
        m,
        grid_k_v1(),
        lda,
    );
    strided_index_is_in_footprint_v1(
        grid_b_depth_v1(lane, component),
        grid_b_col_v1(group_x, lane),
        grid_k_v1(),
        n,
        ldb,
    );
    assert(grid_a_index_v1(group_y, lane, component, lda) + 1 <= a_len);
    assert((grid_a_index_v1(group_y, lane, component, lda) + 1) * 2
        <= a_len * 2) by (nonlinear_arith)
        requires grid_a_index_v1(group_y, lane, component, lda) + 1 <= a_len;
    assert(grid_b_index_v1(group_x, lane, component, ldb) + 1 <= b_len);
    assert((grid_b_index_v1(group_x, lane, component, ldb) + 1) * 2
        <= b_len * 2) by (nonlinear_arith)
        requires grid_b_index_v1(group_x, lane, component, ldb) + 1 <= b_len;
}

proof fn one_grid_c_store_is_in_bounds_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    ldc: nat,
    c_len: nat,
)
    requires
        m > 0,
        n > 0,
        m % 16 == 0,
        n % 16 == 0,
        n <= ldc,
        strided_footprint_v1(m, n, ldc) <= c_len,
        c_len * 4 <= grid_u64_max_v1(),
        checked_grid_workgroup_v1(group_x, group_y, m, n),
        lane < 64,
        component < 4,
    ensures
        grid_c_row_v1(group_y, lane, component) < m,
        grid_c_col_v1(group_x, lane) < n,
        grid_c_index_v1(group_x, group_y, lane, component, ldc) < c_len,
        (grid_c_index_v1(group_x, group_y, lane, component, ldc) + 1) * 4
            <= grid_u64_max_v1(),
{
    checked_grid_tile_origins_v1(group_x, group_y, m, n);
    base::accumulator_coordinates_are_bounded_v1(lane, component);
    strided_index_is_in_footprint_v1(
        grid_c_row_v1(group_y, lane, component),
        grid_c_col_v1(group_x, lane),
        m,
        n,
        ldc,
    );
    assert(grid_c_index_v1(group_x, group_y, lane, component, ldc) + 1 <= c_len);
    assert((grid_c_index_v1(group_x, group_y, lane, component, ldc) + 1) * 4
        <= c_len * 4) by (nonlinear_arith)
        requires grid_c_index_v1(group_x, group_y, lane, component, ldc) + 1 <= c_len;
}

/// Each physical lane owns exactly four modeled stores. All four are in the
/// logical MxN output and in the padded C allocation with u64-safe endpoints.
pub proof fn each_grid_lane_four_c_stores_are_in_bounds_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    m: nat,
    n: nat,
    lda: nat,
    ldb: nat,
    ldc: nat,
    a_len: nat,
    b_len: nat,
    c_len: nat,
)
    requires
        checked_grid_problem_v1(m, n, lda, ldb, ldc, a_len, b_len, c_len),
        checked_grid_workgroup_v1(group_x, group_y, m, n),
        lane < grid_wave_lanes_v1(),
    ensures
        grid_c_index_v1(group_x, group_y, lane, 0, ldc) < c_len,
        grid_c_index_v1(group_x, group_y, lane, 1, ldc) < c_len,
        grid_c_index_v1(group_x, group_y, lane, 2, ldc) < c_len,
        grid_c_index_v1(group_x, group_y, lane, 3, ldc) < c_len,
        (grid_c_index_v1(group_x, group_y, lane, 0, ldc) + 1) * 4
            <= grid_u64_max_v1(),
        (grid_c_index_v1(group_x, group_y, lane, 1, ldc) + 1) * 4
            <= grid_u64_max_v1(),
        (grid_c_index_v1(group_x, group_y, lane, 2, ldc) + 1) * 4
            <= grid_u64_max_v1(),
        (grid_c_index_v1(group_x, group_y, lane, 3, ldc) + 1) * 4
            <= grid_u64_max_v1(),
{
    one_grid_c_store_is_in_bounds_v1(group_x, group_y, lane, 0, m, n, ldc, c_len);
    one_grid_c_store_is_in_bounds_v1(group_x, group_y, lane, 1, m, n, ldc, c_len);
    one_grid_c_store_is_in_bounds_v1(group_x, group_y, lane, 2, m, n, ldc, c_len);
    one_grid_c_store_is_in_bounds_v1(group_x, group_y, lane, 3, m, n, ldc, c_len);
}

/// Unequal workgroup/lane/component tuples own disjoint C elements even when
/// ldc contains row padding.
pub proof fn distinct_grid_invocations_own_disjoint_c_v1(
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
    lda: nat,
    ldb: nat,
    ldc: nat,
    a_len: nat,
    b_len: nat,
    c_len: nat,
)
    requires
        checked_grid_problem_v1(m, n, lda, ldb, ldc, a_len, b_len, c_len),
        checked_grid_workgroup_v1(left_group_x, left_group_y, m, n),
        checked_grid_workgroup_v1(right_group_x, right_group_y, m, n),
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        left_group_x != right_group_x
            || left_group_y != right_group_y
            || left_lane != right_lane
            || left_component != right_component,
    ensures
        grid_c_index_v1(
            left_group_x, left_group_y, left_lane, left_component, ldc,
        ) != grid_c_index_v1(
            right_group_x, right_group_y, right_lane, right_component, ldc,
        ),
{
    one_grid_c_store_is_in_bounds_v1(
        left_group_x, left_group_y, left_lane, left_component, m, n, ldc, c_len,
    );
    one_grid_c_store_is_in_bounds_v1(
        right_group_x, right_group_y, right_lane, right_component, m, n, ldc, c_len,
    );

    let left_row = grid_c_row_v1(left_group_y, left_lane, left_component);
    let left_col = grid_c_col_v1(left_group_x, left_lane);
    let right_row = grid_c_row_v1(right_group_y, right_lane, right_component);
    let right_col = grid_c_col_v1(right_group_x, right_lane);

    if left_group_y != right_group_y {
        if left_group_y < right_group_y {
            assert(left_group_y * 16 + 16 <= right_group_y * 16)
                by (nonlinear_arith)
                requires left_group_y + 1 <= right_group_y;
            assert(left_row < right_row);
        } else {
            assert(right_group_y * 16 + 16 <= left_group_y * 16)
                by (nonlinear_arith)
                requires right_group_y + 1 <= left_group_y;
            assert(right_row < left_row);
        }
    } else if left_group_x != right_group_x {
        if left_group_x < right_group_x {
            assert(left_group_x * 16 + 16 <= right_group_x * 16)
                by (nonlinear_arith)
                requires left_group_x + 1 <= right_group_x;
            assert(left_col < right_col);
        } else {
            assert(right_group_x * 16 + 16 <= left_group_x * 16)
                by (nonlinear_arith)
                requires right_group_x + 1 <= left_group_x;
            assert(right_col < left_col);
        }
    } else {
        assert(left_lane != right_lane || left_component != right_component);
        base::lane_component_register_maps_are_injective_v1(
            left_lane, left_component, right_lane, right_component,
        );
        assert(left_row != right_row || left_col != right_col);
    }
    strided_coordinates_are_injective_v1(
        left_row, left_col, right_row, right_col, ldc,
    );
}

/// Barrier arrivals are modeled independently for one checked workgroup.
/// No cross-workgroup barrier or ordering assumption is introduced.
pub open spec fn grid_lane_reaches_publish_barrier_v1(lane: nat) -> bool {
    lane < grid_wave_lanes_v1()
}

pub open spec fn grid_workgroup_barrier_arrivals_v1(
    arrived: Seq<bool>,
    group_x: nat,
    group_y: nat,
    m: nat,
    n: nat,
) -> bool {
    &&& checked_grid_workgroup_v1(group_x, group_y, m, n)
    &&& arrived.len() == grid_wave_lanes_v1()
    &&& forall |lane: nat| lane < grid_wave_lanes_v1() ==>
        arrived[lane as int] == grid_lane_reaches_publish_barrier_v1(lane)
}

/// Fixed K=16 retains Slice 1's converged publish barrier: every lane stages
/// four A and four B values before reaching the same workgroup-local barrier.
pub proof fn grid_slice1_barrier_converges_for_one_workgroup_v1(
    arrived: Seq<bool>,
    group_x: nat,
    group_y: nat,
    lane: nat,
    m: nat,
    n: nat,
)
    requires
        grid_workgroup_barrier_arrivals_v1(arrived, group_x, group_y, m, n),
        lane < grid_wave_lanes_v1(),
    ensures
        arrived[lane as int],
        grid_lane_reaches_publish_barrier_v1(lane),
{
    assert(forall |physical_lane: nat| physical_lane < 64 ==>
        arrived[physical_lane as int]
            == grid_lane_reaches_publish_barrier_v1(physical_lane));
    assert(arrived[lane as int] == grid_lane_reaches_publish_barrier_v1(lane));
    assert(grid_lane_reaches_publish_barrier_v1(lane));
}

} // verus!
