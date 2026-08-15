use vstd::arithmetic::div_mod::{
    lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse,
};
use vstd::prelude::*;

#[path = "tiled_gemm_host_contract.rs"]
mod base;

verus! {

/// Slice 4 is a bounded source-level model, not a source/backend/hardware
/// refinement result. It models mathematical real values and exact real
/// arithmetic only. It makes no claim about IEEE rounding, NaNs, infinities,
/// signed zero, emitted machine code, or hardware behavior.
pub open spec fn edges_max_extent_v1() -> nat { 32 }
pub open spec fn edges_tile_extent_v1() -> nat { 16 }
pub open spec fn edges_wave_lanes_v1() -> nat { 64 }
pub open spec fn edges_components_per_lane_v1() -> nat { 4 }

pub open spec fn edges_tile_count_v1(value: nat) -> nat {
    value / 16 + if value % 16 == 0 { 0nat } else { 1nat }
}

pub open spec fn bounded_positive_edges_problem_v1(
    a: Seq<real>,
    b: Seq<real>,
    c_input: Seq<real>,
    m: nat,
    n: nat,
    k: nat,
) -> bool {
    &&& 1 <= m <= edges_max_extent_v1()
    &&& 1 <= n <= edges_max_extent_v1()
    &&& 1 <= k <= edges_max_extent_v1()
    &&& a.len() == m * k
    &&& b.len() == k * n
    &&& c_input.len() == m * n
}

/// Empty output follows the current host contract: no dispatch and no operand
/// access, regardless of K. This positive edge model has no empty dispatch.
pub open spec fn edges_empty_output_v1(m: nat, n: nat) -> bool {
    m == 0 || n == 0
}

pub open spec fn edges_empty_output_operand_accesses_v1(
    m: nat,
    n: nat,
    _k: nat,
) -> nat {
    if edges_empty_output_v1(m, n) { 0 } else { 1 }
}

pub proof fn edges_empty_output_is_no_dispatch_no_access_v1(
    m: nat,
    n: nat,
    k: nat,
)
    requires edges_empty_output_v1(m, n),
    ensures edges_empty_output_operand_accesses_v1(m, n, k) == 0,
{
}

/// The current host contract handles nonempty K=0 only after requiring fully
/// tiled M/N, and fills C with positive zero without reading A or B. General
/// alpha/beta K=0 semantics are therefore outside this positive-K milestone.
pub open spec fn edges_legacy_zero_k_host_fill_v1(m: nat, n: nat, k: nat) -> bool {
    &&& m > 0
    &&& n > 0
    &&& m % 16 == 0
    &&& n % 16 == 0
    &&& k == 0
}

pub open spec fn edges_zero_k_host_operand_accesses_v1(
    m: nat,
    n: nat,
    k: nat,
) -> nat {
    if edges_legacy_zero_k_host_fill_v1(m, n, k) { 0 } else { 1 }
}

pub proof fn edges_legacy_zero_k_fill_reads_no_a_or_b_v1(
    m: nat,
    n: nat,
    k: nat,
)
    requires edges_legacy_zero_k_host_fill_v1(m, n, k),
    ensures edges_zero_k_host_operand_accesses_v1(m, n, k) == 0,
{
}

pub open spec fn edges_checked_workgroup_v1(
    group_x: nat,
    group_y: nat,
    m: nat,
    n: nat,
) -> bool {
    group_x < edges_tile_count_v1(n) && group_y < edges_tile_count_v1(m)
}

pub open spec fn edges_phase_count_v1(k: nat) -> nat {
    edges_tile_count_v1(k)
}

pub open spec fn edges_a_row_v1(group_y: nat, lane: nat) -> nat {
    group_y * 16 + base::a_register_row_v1(lane)
}

pub open spec fn edges_a_depth_v1(
    phase: nat,
    lane: nat,
    component: nat,
) -> nat {
    phase * 16 + base::a_register_depth_v1(lane, component)
}

pub open spec fn edges_b_depth_v1(
    phase: nat,
    lane: nat,
    component: nat,
) -> nat {
    phase * 16 + base::b_register_depth_v1(lane, component)
}

pub open spec fn edges_b_col_v1(group_x: nat, lane: nat) -> nat {
    group_x * 16 + base::b_register_col_v1(lane)
}

pub open spec fn edges_c_row_v1(
    group_y: nat,
    lane: nat,
    component: nat,
) -> nat {
    group_y * 16 + base::accumulator_row_v1(lane, component)
}

pub open spec fn edges_c_col_v1(group_x: nat, lane: nat) -> nat {
    group_x * 16 + base::accumulator_col_v1(lane)
}

pub open spec fn edges_a_index_v1(
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    k: nat,
) -> nat {
    edges_a_row_v1(group_y, lane) * k
        + edges_a_depth_v1(phase, lane, component)
}

pub open spec fn edges_b_index_v1(
    group_x: nat,
    phase: nat,
    lane: nat,
    component: nat,
    n: nat,
) -> nat {
    edges_b_depth_v1(phase, lane, component) * n
        + edges_b_col_v1(group_x, lane)
}

pub open spec fn edges_c_index_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    n: nat,
) -> nat {
    edges_c_row_v1(group_y, lane, component) * n
        + edges_c_col_v1(group_x, lane)
}

pub open spec fn edges_a_load_enabled_v1(
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    k: nat,
) -> bool {
    edges_a_row_v1(group_y, lane) < m
        && edges_a_depth_v1(phase, lane, component) < k
}

pub open spec fn edges_b_load_enabled_v1(
    group_x: nat,
    phase: nat,
    lane: nat,
    component: nat,
    n: nat,
    k: nat,
) -> bool {
    edges_b_depth_v1(phase, lane, component) < k
        && edges_b_col_v1(group_x, lane) < n
}

pub open spec fn edges_c_store_enabled_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
) -> bool {
    edges_c_row_v1(group_y, lane, component) < m
        && edges_c_col_v1(group_x, lane) < n
}

pub open spec fn edges_a_issues_global_load_v1(
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    k: nat,
) -> bool {
    edges_a_load_enabled_v1(group_y, phase, lane, component, m, k)
}

pub open spec fn edges_b_issues_global_load_v1(
    group_x: nat,
    phase: nat,
    lane: nat,
    component: nat,
    n: nat,
    k: nat,
) -> bool {
    edges_b_load_enabled_v1(group_x, phase, lane, component, n, k)
}

/// The C input read and C output store share one output-coordinate predicate.
pub open spec fn edges_c_issues_global_access_v1(
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
) -> bool {
    edges_c_store_enabled_v1(group_x, group_y, lane, component, m, n)
}

pub open spec fn edges_a_staged_value_v1(
    a: Seq<real>,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    k: nat,
) -> real
    recommends
        a.len() == m * k,
        lane < 64,
        component < 4,
{
    if edges_a_load_enabled_v1(group_y, phase, lane, component, m, k) {
        a[edges_a_index_v1(group_y, phase, lane, component, k) as int]
    } else {
        0real
    }
}

pub open spec fn edges_b_staged_value_v1(
    b: Seq<real>,
    group_x: nat,
    phase: nat,
    lane: nat,
    component: nat,
    n: nat,
    k: nat,
) -> real
    recommends
        b.len() == k * n,
        lane < 64,
        component < 4,
{
    if edges_b_load_enabled_v1(group_x, phase, lane, component, n, k) {
        b[edges_b_index_v1(group_x, phase, lane, component, n) as int]
    } else {
        0real
    }
}

proof fn row_major_index_is_bounded_v1(
    row: nat,
    column: nat,
    rows: nat,
    columns: nat,
)
    requires row < rows, column < columns,
    ensures row * columns + column < rows * columns,
{
    assert(row * columns + column < (row + 1) * columns)
        by (nonlinear_arith)
        requires column < columns;
    assert((row + 1) * columns <= rows * columns)
        by (nonlinear_arith)
        requires row + 1 <= rows;
}

/// Every per-lane cooperative A/B operation either performs an in-allocation
/// global load or performs no global access and stages exact-real zero.
pub proof fn each_lane_predicated_global_load_is_bounded_or_zero_filled_v1(
    a: Seq<real>,
    b: Seq<real>,
    c_input: Seq<real>,
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
        bounded_positive_edges_problem_v1(a, b, c_input, m, n, k),
        edges_checked_workgroup_v1(group_x, group_y, m, n),
        phase < edges_phase_count_v1(k),
        lane < edges_wave_lanes_v1(),
        component < edges_components_per_lane_v1(),
    ensures
        edges_a_issues_global_load_v1(group_y, phase, lane, component, m, k)
            ==> edges_a_index_v1(group_y, phase, lane, component, k) < a.len(),
        !edges_a_issues_global_load_v1(group_y, phase, lane, component, m, k)
            ==> edges_a_staged_value_v1(a, group_y, phase, lane, component, m, k)
                == 0real,
        edges_b_issues_global_load_v1(group_x, phase, lane, component, n, k)
            ==> edges_b_index_v1(group_x, phase, lane, component, n) < b.len(),
        !edges_b_issues_global_load_v1(group_x, phase, lane, component, n, k)
            ==> edges_b_staged_value_v1(b, group_x, phase, lane, component, n, k)
                == 0real,
{
    base::a_register_coordinates_are_bounded_v1(lane, component);
    base::b_register_coordinates_are_bounded_v1(lane, component);
    if edges_a_load_enabled_v1(group_y, phase, lane, component, m, k) {
        row_major_index_is_bounded_v1(
            edges_a_row_v1(group_y, lane),
            edges_a_depth_v1(phase, lane, component),
            m,
            k,
        );
    }
    if edges_b_load_enabled_v1(group_x, phase, lane, component, n, k) {
        row_major_index_is_bounded_v1(
            edges_b_depth_v1(phase, lane, component),
            edges_b_col_v1(group_x, lane),
            k,
            n,
        );
    }
}

/// Predicate-off lanes issue neither the C input read nor the C output store.
/// Predicate-on lanes access exactly one element inside the packed MxN C area.
pub proof fn each_lane_predicated_c_access_has_no_oob_store_v1(
    a: Seq<real>,
    b: Seq<real>,
    c_input: Seq<real>,
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        bounded_positive_edges_problem_v1(a, b, c_input, m, n, k),
        edges_checked_workgroup_v1(group_x, group_y, m, n),
        lane < edges_wave_lanes_v1(),
        component < edges_components_per_lane_v1(),
    ensures
        edges_c_issues_global_access_v1(group_x, group_y, lane, component, m, n)
            ==> edges_c_index_v1(group_x, group_y, lane, component, n)
                < c_input.len(),
        !edges_c_store_enabled_v1(group_x, group_y, lane, component, m, n)
            ==> !edges_c_issues_global_access_v1(
                group_x, group_y, lane, component, m, n,
            ),
{
    base::accumulator_coordinates_are_bounded_v1(lane, component);
    if edges_c_store_enabled_v1(group_x, group_y, lane, component, m, n) {
        row_major_index_is_bounded_v1(
            edges_c_row_v1(group_y, lane, component),
            edges_c_col_v1(group_x, lane),
            m,
            n,
        );
    }
}

proof fn packed_coordinate_v1(block: nat, inner: nat, radix: nat)
    requires radix > 0, inner < radix,
    ensures
        (block * radix + inner) / radix == block,
        (block * radix + inner) % radix == inner,
{
    lemma_fundamental_div_mod_converse(
        (block * radix + inner) as int,
        radix as int,
        block as int,
        inner as int,
    );
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
        left_row * columns + left_col == right_row * columns + right_col,
    ensures left_row == right_row, left_col == right_col,
{
    packed_coordinate_v1(left_row, left_col, columns);
    packed_coordinate_v1(right_row, right_col, columns);
}

/// Unequal physical output owners cannot store the same valid MxN element,
/// including across partial M/N tiles.
pub proof fn distinct_valid_edge_output_owners_are_disjoint_v1(
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
)
    requires
        1 <= m <= edges_max_extent_v1(),
        1 <= n <= edges_max_extent_v1(),
        edges_checked_workgroup_v1(left_group_x, left_group_y, m, n),
        edges_checked_workgroup_v1(right_group_x, right_group_y, m, n),
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        edges_c_store_enabled_v1(
            left_group_x, left_group_y, left_lane, left_component, m, n,
        ),
        edges_c_store_enabled_v1(
            right_group_x, right_group_y, right_lane, right_component, m, n,
        ),
        left_group_x != right_group_x
            || left_group_y != right_group_y
            || left_lane != right_lane
            || left_component != right_component,
    ensures
        edges_c_index_v1(
            left_group_x, left_group_y, left_lane, left_component, n,
        ) != edges_c_index_v1(
            right_group_x, right_group_y, right_lane, right_component, n,
        ),
{
    let left_row = edges_c_row_v1(left_group_y, left_lane, left_component);
    let left_col = edges_c_col_v1(left_group_x, left_lane);
    let right_row = edges_c_row_v1(right_group_y, right_lane, right_component);
    let right_col = edges_c_col_v1(right_group_x, right_lane);

    if edges_c_index_v1(
        left_group_x, left_group_y, left_lane, left_component, n,
    ) == edges_c_index_v1(
        right_group_x, right_group_y, right_lane, right_component, n,
    ) {
        row_major_coordinates_are_injective_v1(
            left_row, left_col, right_row, right_col, n,
        );
        base::accumulator_coordinates_are_bounded_v1(left_lane, left_component);
        base::accumulator_coordinates_are_bounded_v1(right_lane, right_component);
        packed_coordinate_v1(
            left_group_y,
            base::accumulator_row_v1(left_lane, left_component),
            16,
        );
        packed_coordinate_v1(
            right_group_y,
            base::accumulator_row_v1(right_lane, right_component),
            16,
        );
        packed_coordinate_v1(
            left_group_x,
            base::accumulator_col_v1(left_lane),
            16,
        );
        packed_coordinate_v1(
            right_group_x,
            base::accumulator_col_v1(right_lane),
            16,
        );
        assert(left_group_x == right_group_x);
        assert(left_group_y == right_group_y);
        if left_lane != right_lane || left_component != right_component {
            base::lane_component_register_maps_are_injective_v1(
                left_lane,
                left_component,
                right_lane,
                right_component,
            );
            assert(false);
        }
        assert(false);
    }
}

proof fn tile_count_covers_positive_extent_v1(value: nat)
    requires value > 0,
    ensures
        edges_tile_count_v1(value) > 0,
        value <= edges_tile_count_v1(value) * 16,
{
    lemma_fundamental_div_mod(value as int, 16);
    assert(value == (value / 16) * 16 + value % 16);
    assert(value % 16 < 16);
    if value % 16 == 0 {
        if value / 16 == 0 {
            assert(value == 0);
            assert(false);
        }
    } else {
        assert(value <= (value / 16 + 1) * 16) by (nonlinear_arith)
            requires value % 16 < 16;
    }
}

/// Every valid K depth has exactly one `(phase, offset)` in the ceil-divided
/// phase space. The final phase's offsets at or beyond K are not valid depths.
pub proof fn each_valid_k_depth_has_exactly_one_tiled_position_v1(
    k: nat,
    depth: nat,
)
    requires 1 <= k <= edges_max_extent_v1(), depth < k,
    ensures
        depth / 16 < edges_phase_count_v1(k),
        depth % 16 < 16,
        (depth / 16) * 16 + depth % 16 == depth,
{
    lemma_fundamental_div_mod(depth as int, 16);
    lemma_fundamental_div_mod(k as int, 16);
    assert(depth == (depth / 16) * 16 + depth % 16);
    assert(k == (k / 16) * 16 + k % 16);
    assert(depth % 16 < 16);
    assert(k % 16 < 16);
    if k % 16 == 0 {
        if depth / 16 >= k / 16 {
            assert(depth >= (depth / 16) * 16) by (nonlinear_arith)
                requires depth == (depth / 16) * 16 + depth % 16;
            assert((depth / 16) * 16 >= (k / 16) * 16)
                by (nonlinear_arith)
                requires depth / 16 >= k / 16;
            assert(depth >= k);
            assert(false);
        }
    } else if depth / 16 > k / 16 {
        assert(depth >= (depth / 16) * 16) by (nonlinear_arith)
            requires depth == (depth / 16) * 16 + depth % 16;
        assert((depth / 16) * 16 >= (k / 16 + 1) * 16)
            by (nonlinear_arith)
            requires depth / 16 >= k / 16 + 1;
        assert((k / 16 + 1) * 16 > k) by (nonlinear_arith)
            requires
                k == (k / 16) * 16 + k % 16,
                k % 16 < 16;
        assert(false);
    }
}

pub proof fn valid_k_depth_tiled_position_is_unique_v1(
    k: nat,
    depth: nat,
    other_phase: nat,
    other_offset: nat,
)
    requires
        1 <= k <= edges_max_extent_v1(),
        depth < k,
        other_phase < edges_phase_count_v1(k),
        other_offset < 16,
        other_phase * 16 + other_offset == depth,
    ensures
        other_phase == depth / 16,
        other_offset == depth % 16,
{
    packed_coordinate_v1(other_phase, other_offset, 16);
}

pub open spec fn edges_a_tile_value_v1(
    a: Seq<real>,
    group_y: nat,
    phase: nat,
    tile_row: nat,
    tile_depth: nat,
    m: nat,
    k: nat,
) -> real
    recommends a.len() == m * k, tile_row < 16, tile_depth < 16,
{
    let row = group_y * 16 + tile_row;
    let depth = phase * 16 + tile_depth;
    if row < m && depth < k {
        a[(row * k + depth) as int]
    } else {
        0real
    }
}

pub open spec fn edges_b_tile_value_v1(
    b: Seq<real>,
    group_x: nat,
    phase: nat,
    tile_depth: nat,
    tile_col: nat,
    n: nat,
    k: nat,
) -> real
    recommends b.len() == k * n, tile_depth < 16, tile_col < 16,
{
    let depth = phase * 16 + tile_depth;
    let column = group_x * 16 + tile_col;
    if depth < k && column < n {
        b[(depth * n + column) as int]
    } else {
        0real
    }
}

/// Any OOB logical A/B element in a partial M/N/K tile is staged as exact-real
/// zero. Valid elements retain their corresponding packed global value.
pub proof fn every_oob_tile_element_is_zero_filled_v1(
    a: Seq<real>,
    b: Seq<real>,
    group_x: nat,
    group_y: nat,
    phase: nat,
    tile_row: nat,
    tile_depth: nat,
    tile_col: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        a.len() == m * k,
        b.len() == k * n,
        tile_row < 16,
        tile_depth < 16,
        tile_col < 16,
    ensures
        (group_y * 16 + tile_row >= m || phase * 16 + tile_depth >= k)
            ==> edges_a_tile_value_v1(
                a, group_y, phase, tile_row, tile_depth, m, k,
            ) == 0real,
        (phase * 16 + tile_depth >= k || group_x * 16 + tile_col >= n)
            ==> edges_b_tile_value_v1(
                b, group_x, phase, tile_depth, tile_col, n, k,
            ) == 0real,
{
}

/// Barrier control is a function only of physical lane and phase bounds. The
/// load/store predicates and barrier kind are deliberately ignored.
pub open spec fn edges_lane_reaches_phase_barrier_v1(
    lane: nat,
    phase: nat,
    phase_count: nat,
    _is_publish: bool,
    _a_load_enabled: bool,
    _b_load_enabled: bool,
    _c_store_enabled: bool,
) -> bool {
    lane < edges_wave_lanes_v1() && phase < phase_count
}

/// Every physical lane reaches both barriers in every phase, even if any or
/// all of its global loads and stores are predicated off. Barrier convergence
/// is therefore independent of edge predicates.
pub proof fn barrier_convergence_is_independent_of_predicates_v1(
    lane: nat,
    phase: nat,
    phase_count: nat,
    left_a: bool,
    left_b: bool,
    left_c: bool,
    right_a: bool,
    right_b: bool,
    right_c: bool,
)
    requires lane < 64, phase < phase_count,
    ensures
        edges_lane_reaches_phase_barrier_v1(
            lane, phase, phase_count, true, left_a, left_b, left_c,
        ),
        edges_lane_reaches_phase_barrier_v1(
            lane, phase, phase_count, false, left_a, left_b, left_c,
        ),
        edges_lane_reaches_phase_barrier_v1(
            lane, phase, phase_count, true, left_a, left_b, left_c,
        ) == edges_lane_reaches_phase_barrier_v1(
            lane, phase, phase_count, true, right_a, right_b, right_c,
        ),
        edges_lane_reaches_phase_barrier_v1(
            lane, phase, phase_count, false, left_a, left_b, left_c,
        ) == edges_lane_reaches_phase_barrier_v1(
            lane, phase, phase_count, false, right_a, right_b, right_c,
        ),
{
}

pub open spec fn edges_exact_product_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    m: nat,
    n: nat,
    k: nat,
    end: nat,
) -> real
    recommends
        a.len() == m * k,
        b.len() == k * n,
        row < m,
        column < n,
        end <= k,
    decreases end,
{
    if end == 0 {
        0real
    } else {
        edges_exact_product_prefix_v1(
            a, b, row, column, m, n, k, (end - 1) as nat,
        ) + a[(row * k + (end - 1) as nat) as int]
            * b[((end - 1) as nat * n + column) as int]
    }
}

pub open spec fn edges_zero_filled_product_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    m: nat,
    n: nat,
    k: nat,
    padded_end: nat,
) -> real
    recommends
        a.len() == m * k,
        b.len() == k * n,
        row < m,
        column < n,
    decreases padded_end,
{
    if padded_end == 0 {
        0real
    } else {
        let depth = (padded_end - 1) as nat;
        edges_zero_filled_product_prefix_v1(
            a, b, row, column, m, n, k, depth,
        ) + if depth < k {
            a[(row * k + depth) as int] * b[(depth * n + column) as int]
        } else {
            0real
        }
    }
}

proof fn zero_filled_prefix_matches_exact_before_k_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    m: nat,
    n: nat,
    k: nat,
    end: nat,
)
    requires
        a.len() == m * k,
        b.len() == k * n,
        row < m,
        column < n,
        end <= k,
    ensures
        edges_zero_filled_product_prefix_v1(a, b, row, column, m, n, k, end)
            == edges_exact_product_prefix_v1(a, b, row, column, m, n, k, end),
    decreases end,
{
    if end > 0 {
        row_major_index_is_bounded_v1(row, (end - 1) as nat, m, k);
        row_major_index_is_bounded_v1((end - 1) as nat, column, k, n);
        zero_filled_prefix_matches_exact_before_k_v1(
            a, b, row, column, m, n, k, (end - 1) as nat,
        );
    }
}

proof fn zero_filled_suffix_contributes_zero_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    m: nat,
    n: nat,
    k: nat,
    padded_end: nat,
)
    requires
        a.len() == m * k,
        b.len() == k * n,
        row < m,
        column < n,
        k <= padded_end,
    ensures
        edges_zero_filled_product_prefix_v1(
            a, b, row, column, m, n, k, padded_end,
        ) == edges_exact_product_prefix_v1(a, b, row, column, m, n, k, k),
    decreases padded_end - k,
{
    if padded_end == k {
        zero_filled_prefix_matches_exact_before_k_v1(
            a, b, row, column, m, n, k, k,
        );
    } else {
        zero_filled_suffix_contributes_zero_v1(
            a, b, row, column, m, n, k, (padded_end - 1) as nat,
        );
        assert((padded_end - 1) as nat >= k);
    }
}

pub open spec fn edges_tiled_product_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    m: nat,
    n: nat,
    k: nat,
) -> real
    recommends
        a.len() == m * k,
        b.len() == k * n,
        row < m,
        column < n,
{
    edges_zero_filled_product_prefix_v1(
        a,
        b,
        row,
        column,
        m,
        n,
        k,
        edges_phase_count_v1(k) * 16,
    )
}

/// Ceil-divided K phases include every valid depth exactly once and contribute
/// exact zero for all padded tail offsets.
pub proof fn k_tail_contributes_every_valid_depth_exactly_once_v1(
    a: Seq<real>,
    b: Seq<real>,
    c_input: Seq<real>,
    row: nat,
    column: nat,
    m: nat,
    n: nat,
    k: nat,
)
    requires
        bounded_positive_edges_problem_v1(a, b, c_input, m, n, k),
        row < m,
        column < n,
    ensures
        edges_tiled_product_v1(a, b, row, column, m, n, k)
            == edges_exact_product_prefix_v1(a, b, row, column, m, n, k, k),
{
    tile_count_covers_positive_extent_v1(k);
    zero_filled_suffix_contributes_zero_v1(
        a,
        b,
        row,
        column,
        m,
        n,
        k,
        edges_phase_count_v1(k) * 16,
    );
}

/// Alpha and beta are arbitrary mathematical reals. This is an exact-real
/// abstraction, not an IEEE-F32 operation or a bitwise numerical contract.
pub open spec fn edges_exact_alpha_beta_v1(
    product: real,
    c_input: real,
    alpha: real,
    beta: real,
) -> real {
    alpha * product + beta * c_input
}

pub open spec fn edges_lane_output_v1(
    a: Seq<real>,
    b: Seq<real>,
    c_input: Seq<real>,
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
    alpha: real,
    beta: real,
) -> real
    recommends
        bounded_positive_edges_problem_v1(a, b, c_input, m, n, k),
        edges_c_store_enabled_v1(group_x, group_y, lane, component, m, n),
{
    edges_exact_alpha_beta_v1(
        edges_tiled_product_v1(
            a,
            b,
            edges_c_row_v1(group_y, lane, component),
            edges_c_col_v1(group_x, lane),
            m,
            n,
            k,
        ),
        c_input[edges_c_index_v1(group_x, group_y, lane, component, n) as int],
        alpha,
        beta,
    )
}

/// Every predicate-on lane result has exactly `alpha*Cproduct + beta*Cinput`
/// under the explicit exact-real abstraction, including a partial K phase.
pub proof fn each_valid_edge_output_has_exact_alpha_beta_v1(
    a: Seq<real>,
    b: Seq<real>,
    c_input: Seq<real>,
    group_x: nat,
    group_y: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
    alpha: real,
    beta: real,
)
    requires
        bounded_positive_edges_problem_v1(a, b, c_input, m, n, k),
        edges_checked_workgroup_v1(group_x, group_y, m, n),
        lane < 64,
        component < 4,
        edges_c_store_enabled_v1(group_x, group_y, lane, component, m, n),
    ensures
        edges_lane_output_v1(
            a,
            b,
            c_input,
            group_x,
            group_y,
            lane,
            component,
            m,
            n,
            k,
            alpha,
            beta,
        ) == alpha * edges_exact_product_prefix_v1(
            a,
            b,
            edges_c_row_v1(group_y, lane, component),
            edges_c_col_v1(group_x, lane),
            m,
            n,
            k,
            k,
        ) + beta * c_input[
            edges_c_index_v1(group_x, group_y, lane, component, n) as int
        ],
{
    each_lane_predicated_c_access_has_no_oob_store_v1(
        a, b, c_input, group_x, group_y, lane, component, m, n, k,
    );
    k_tail_contributes_every_valid_depth_exactly_once_v1(
        a,
        b,
        c_input,
        edges_c_row_v1(group_y, lane, component),
        edges_c_col_v1(group_x, lane),
        m,
        n,
        k,
    );
}

/// The legacy K=0 positive-zero fill agrees with the generalized exact-real
/// formula exactly when the otherwise-unmodeled `beta*Cinput` term is zero.
pub proof fn legacy_zero_k_matches_alpha_beta_only_when_beta_c_is_zero_v1(
    alpha: real,
    beta: real,
    c_input: real,
)
    ensures
        edges_exact_alpha_beta_v1(0real, c_input, alpha, beta) == 0real
            <==> beta * c_input == 0real,
{
}

} // verus!
