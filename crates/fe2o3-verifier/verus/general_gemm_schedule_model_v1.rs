use vstd::arithmetic::div_mod::{
    lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse, lemma_mod_bound,
};
use vstd::prelude::*;

verus! {

/// Solver model shared by the two issue #138 schedule instantiations. Values
/// are exact mathematical reals. The machine-refinement gate must separately
/// connect emitted gfx942 operations to the declared BF16/F32 policy.
pub enum GeneralGemmScheduleModelV1 {
    ReferenceWave64Xor4,
    VectorizedAOnlyBf16GlobalTransfer,
}

pub open spec fn tile_v1() -> nat { 16 }
pub open spec fn lanes_v1() -> nat { 64 }
pub open spec fn components_v1() -> nat { 4 }

pub open spec fn ceil_div_16_v1(value: nat) -> nat {
    value / 16 + if value % 16 == 0 { 0nat } else { 1nat }
}

pub open spec fn extent_v1(rows: nat, columns: nat, stride: nat) -> nat {
    if rows == 0 || columns == 0 { 0 } else { (rows - 1) as nat * stride + columns }
}

pub open spec fn checked_problem_v1(
    a: Seq<real>,
    b: Seq<real>,
    c: Seq<real>,
    m: nat,
    n: nat,
    k: nat,
    lda: nat,
    ldb: nat,
    ldc: nat,
) -> bool {
    &&& lda >= k
    &&& ldb >= n
    &&& ldc >= n
    &&& a.len() == extent_v1(m, k, lda)
    &&& b.len() == extent_v1(k, n, ldb)
    &&& c.len() == extent_v1(m, n, ldc)
}

pub open spec fn row_major_index_v1(row: nat, column: nat, stride: nat) -> nat {
    row * stride + column
}

pub proof fn checked_row_major_index_is_bounded_v1(
    row: nat,
    column: nat,
    rows: nat,
    columns: nat,
    stride: nat,
)
    requires
        row < rows,
        column < columns,
        stride >= columns,
    ensures
        row_major_index_v1(row, column, stride) < extent_v1(rows, columns, stride),
{
    assert(row * stride + column < row * stride + columns) by (nonlinear_arith)
        requires column < columns;
    assert(row * stride + columns <= (rows - 1) as nat * stride + columns)
        by (nonlinear_arith)
        requires row <= (rows - 1) as nat;
}

pub open spec fn lane_axis_v1(lane: nat) -> nat { lane % 16 }
pub open spec fn lane_depth_base_v1(lane: nat) -> nat { (lane / 16) * 4 }
pub open spec fn output_row_component_v1(lane: nat, component: nat) -> nat {
    (lane / 16) * 4 + component
}

pub proof fn lane_coordinates_are_bounded_v1(lane: nat, component: nat)
    requires lane < lanes_v1(), component < components_v1(),
    ensures
        lane_axis_v1(lane) < 16,
        lane_depth_base_v1(lane) + component < 16,
        output_row_component_v1(lane, component) < 16,
{
    lemma_mod_bound(lane as int, 16);
    lemma_fundamental_div_mod(lane as int, 16);
    assert(lane == 16 * (lane / 16) + lane % 16);
    assert(16 * (lane / 16) == (lane / 16) * 16) by (nonlinear_arith);
    if lane / 16 >= 4 {
        assert((lane / 16) * 16 >= 64) by (nonlinear_arith)
            requires lane / 16 >= 4;
        assert(lane >= 64);
        assert(false);
    }
    assert((lane / 16) * 4 + component < 16) by (nonlinear_arith)
        requires lane / 16 < 4, component < 4;
}

pub open spec fn a_row_v1(group_y: nat, lane: nat) -> nat {
    group_y * 16 + lane_axis_v1(lane)
}

pub open spec fn b_column_v1(group_x: nat, lane: nat) -> nat {
    group_x * 16 + lane_axis_v1(lane)
}

pub open spec fn phase_depth_v1(phase: nat, lane: nat, component: nat) -> nat {
    phase * 16 + lane_depth_base_v1(lane) + component
}

pub open spec fn a_load_enabled_v1(
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    k: nat,
) -> bool {
    a_row_v1(group_y, lane) < m && phase_depth_v1(phase, lane, component) < k
}

pub open spec fn b_load_enabled_v1(
    group_x: nat,
    phase: nat,
    lane: nat,
    component: nat,
    n: nat,
    k: nat,
) -> bool {
    phase_depth_v1(phase, lane, component) < k && b_column_v1(group_x, lane) < n
}

pub open spec fn vector4_a_enabled_v1(
    group_y: nat,
    phase: nat,
    lane: nat,
    m: nat,
    k: nat,
    lda: nat,
) -> bool {
    &&& a_row_v1(group_y, lane) < m
    &&& phase_depth_v1(phase, lane, 3) < k
    &&& row_major_index_v1(
        a_row_v1(group_y, lane), phase_depth_v1(phase, lane, 0), lda,
    ) % 4 == 0
}

/// A vector transfer is enabled only when all four logical A elements are
/// present and its first address is four-element aligned.
pub proof fn vector4_a_transfer_is_bounded_v1(
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    k: nat,
    lda: nat,
)
    requires
        lane < 64,
        component < 4,
        lda >= k,
        vector4_a_enabled_v1(group_y, phase, lane, m, k, lda),
    ensures
        a_load_enabled_v1(group_y, phase, lane, component, m, k),
        row_major_index_v1(
            a_row_v1(group_y, lane), phase_depth_v1(phase, lane, component), lda,
        ) < extent_v1(m, k, lda),
{
    lane_coordinates_are_bounded_v1(lane, component);
    assert(phase_depth_v1(phase, lane, component)
        <= phase_depth_v1(phase, lane, 3)) by (nonlinear_arith)
        requires component <= 3;
    checked_row_major_index_is_bounded_v1(
        a_row_v1(group_y, lane),
        phase_depth_v1(phase, lane, component),
        m,
        k,
        lda,
    );
}

/// Every global address that the schedule model enables is inside the exact
/// row-major extent supplied by `checked_problem_v1`. This theorem covers the
/// model's A, B, and C address arithmetic; it does not establish allocation
/// provenance for an imported kernel.
pub proof fn schedule_modeled_global_accesses_are_bounded_v1(
    schedule: GeneralGemmScheduleModelV1,
    a: Seq<real>,
    b: Seq<real>,
    c: Seq<real>,
    group_x: nat,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
    lda: nat,
    ldb: nat,
    ldc: nat,
)
    requires
        checked_problem_v1(a, b, c, m, n, k, lda, ldb, ldc),
        lane < 64,
        component < 4,
    ensures
        a_load_enabled_v1(group_y, phase, lane, component, m, k) ==>
            row_major_index_v1(
                a_row_v1(group_y, lane), phase_depth_v1(phase, lane, component), lda,
            ) < a.len(),
        b_load_enabled_v1(group_x, phase, lane, component, n, k) ==>
            row_major_index_v1(
                phase_depth_v1(phase, lane, component), b_column_v1(group_x, lane), ldb,
            ) < b.len(),
        output_row_v1(group_y, lane, component) < m
            && output_column_v1(group_x, lane) < n ==>
            row_major_index_v1(
                output_row_v1(group_y, lane, component),
                output_column_v1(group_x, lane),
                ldc,
            ) < c.len(),
{
    match schedule {
        GeneralGemmScheduleModelV1::ReferenceWave64Xor4 => {},
        GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer => {},
    }
    if a_load_enabled_v1(group_y, phase, lane, component, m, k) {
        checked_row_major_index_is_bounded_v1(
            a_row_v1(group_y, lane),
            phase_depth_v1(phase, lane, component),
            m,
            k,
            lda,
        );
    }
    if b_load_enabled_v1(group_x, phase, lane, component, n, k) {
        checked_row_major_index_is_bounded_v1(
            phase_depth_v1(phase, lane, component),
            b_column_v1(group_x, lane),
            k,
            n,
            ldb,
        );
    }
    if output_row_v1(group_y, lane, component) < m
        && output_column_v1(group_x, lane) < n
    {
        checked_row_major_index_is_bounded_v1(
            output_row_v1(group_y, lane, component),
            output_column_v1(group_x, lane),
            m,
            n,
            ldc,
        );
    }
}

pub open spec fn a_staged_value_v1(
    a: Seq<real>,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    k: nat,
    lda: nat,
) -> real {
    if a_load_enabled_v1(group_y, phase, lane, component, m, k) {
        a[row_major_index_v1(
            a_row_v1(group_y, lane), phase_depth_v1(phase, lane, component), lda,
        ) as int]
    } else {
        0real
    }
}

pub open spec fn b_staged_value_v1(
    b: Seq<real>,
    group_x: nat,
    phase: nat,
    lane: nat,
    component: nat,
    n: nat,
    k: nat,
    ldb: nat,
) -> real {
    if b_load_enabled_v1(group_x, phase, lane, component, n, k) {
        b[row_major_index_v1(
            phase_depth_v1(phase, lane, component), b_column_v1(group_x, lane), ldb,
        ) as int]
    } else {
        0real
    }
}

/// Both schedules stage the same logical value. The optimized schedule may
/// obtain four values together only under the full-vector predicate; its
/// scalar masked fallback retains this per-component contract.
pub proof fn schedule_tail_refinement_v1(
    schedule: GeneralGemmScheduleModelV1,
    a: Seq<real>,
    b: Seq<real>,
    group_x: nat,
    group_y: nat,
    phase: nat,
    lane: nat,
    component: nat,
    m: nat,
    n: nat,
    k: nat,
    lda: nat,
    ldb: nat,
)
    requires lane < 64, component < 4,
    ensures
        !a_load_enabled_v1(group_y, phase, lane, component, m, k)
            ==> a_staged_value_v1(a, group_y, phase, lane, component, m, k, lda) == 0real,
        !b_load_enabled_v1(group_x, phase, lane, component, n, k)
            ==> b_staged_value_v1(b, group_x, phase, lane, component, n, k, ldb) == 0real,
{
    match schedule {
        GeneralGemmScheduleModelV1::ReferenceWave64Xor4 => {},
        GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer => {},
    }
}

pub open spec fn output_row_v1(group_y: nat, lane: nat, component: nat) -> nat {
    group_y * 16 + output_row_component_v1(lane, component)
}

pub open spec fn output_column_v1(group_x: nat, lane: nat) -> nat {
    group_x * 16 + lane_axis_v1(lane)
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

/// One `(workgroup,lane,component)` tuple owns each live C coordinate.
pub proof fn schedule_output_region_injective_v1(
    left_group_x: nat,
    left_group_y: nat,
    left_lane: nat,
    left_component: nat,
    right_group_x: nat,
    right_group_y: nat,
    right_lane: nat,
    right_component: nat,
)
    requires
        left_lane < 64,
        right_lane < 64,
        left_component < 4,
        right_component < 4,
        output_row_v1(left_group_y, left_lane, left_component)
            == output_row_v1(right_group_y, right_lane, right_component),
        output_column_v1(left_group_x, left_lane)
            == output_column_v1(right_group_x, right_lane),
    ensures
        left_group_x == right_group_x,
        left_group_y == right_group_y,
        left_lane == right_lane,
        left_component == right_component,
{
    lane_coordinates_are_bounded_v1(left_lane, left_component);
    lane_coordinates_are_bounded_v1(right_lane, right_component);
    packed_coordinate_v1(left_group_x, lane_axis_v1(left_lane), 16);
    packed_coordinate_v1(right_group_x, lane_axis_v1(right_lane), 16);
    packed_coordinate_v1(
        left_group_y, output_row_component_v1(left_lane, left_component), 16,
    );
    packed_coordinate_v1(
        right_group_y, output_row_component_v1(right_lane, right_component), 16,
    );
    assert(lane_axis_v1(left_lane) == lane_axis_v1(right_lane));
    assert(output_row_component_v1(left_lane, left_component)
        == output_row_component_v1(right_lane, right_component));
    packed_coordinate_v1(left_lane / 16, left_component, 4);
    packed_coordinate_v1(right_lane / 16, right_component, 4);
    assert(left_lane / 16 == right_lane / 16);
    assert(left_component == right_component);
    lemma_fundamental_div_mod(left_lane as int, 16);
    lemma_fundamental_div_mod(right_lane as int, 16);
    assert(left_lane == (left_lane / 16) * 16 + lane_axis_v1(left_lane));
    assert(right_lane == (right_lane / 16) * 16 + lane_axis_v1(right_lane));
}

pub open spec fn publish_event_v1(phase: nat) -> nat { phase * 4 + 1 }
pub open spec fn read_event_v1(phase: nat) -> nat { phase * 4 + 2 }
pub open spec fn reuse_event_v1(phase: nat) -> nat { phase * 4 + 3 }
pub open spec fn next_stage_event_v1(phase: nat) -> nat { (phase + 1) * 4 }

/// Both schedules keep the same single-buffered publish/read/reuse lifecycle.
pub proof fn schedule_lds_epoch_correct_v1(phase: nat)
    ensures
        publish_event_v1(phase) < read_event_v1(phase),
        read_event_v1(phase) < reuse_event_v1(phase),
        reuse_event_v1(phase) < next_stage_event_v1(phase),
{
}

pub open spec fn lane_reaches_barrier_v1(lane: nat, phase: nat, phases: nat) -> bool {
    lane < 64 && phase < phases
}

pub proof fn schedule_barrier_convergent_v1(
    left_lane: nat,
    right_lane: nat,
    phase: nat,
    phases: nat,
)
    requires left_lane < 64, right_lane < 64, phase < phases,
    ensures
        lane_reaches_barrier_v1(left_lane, phase, phases),
        lane_reaches_barrier_v1(right_lane, phase, phases),
{
}

pub open spec fn dot_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    lda: nat,
    ldb: nat,
    depth: nat,
) -> real
    decreases depth,
{
    if depth == 0 {
        0real
    } else {
        dot_prefix_v1(a, b, row, column, lda, ldb, (depth - 1) as nat)
            + a[row_major_index_v1(row, (depth - 1) as nat, lda) as int]
                * b[row_major_index_v1((depth - 1) as nat, column, ldb) as int]
    }
}

pub open spec fn scheduled_prefix_v1(
    schedule: GeneralGemmScheduleModelV1,
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    lda: nat,
    ldb: nat,
    depth: nat,
) -> real
    decreases depth,
{
    if depth == 0 {
        0real
    } else {
        let prior = scheduled_prefix_v1(
            schedule, a, b, row, column, lda, ldb, (depth - 1) as nat,
        );
        match schedule {
            GeneralGemmScheduleModelV1::ReferenceWave64Xor4 =>
                prior + a[row_major_index_v1(row, (depth - 1) as nat, lda) as int]
                    * b[row_major_index_v1((depth - 1) as nat, column, ldb) as int],
            GeneralGemmScheduleModelV1::VectorizedAOnlyBf16GlobalTransfer =>
                prior + a[row_major_index_v1(row, (depth - 1) as nat, lda) as int]
                    * b[row_major_index_v1((depth - 1) as nat, column, ldb) as int],
        }
    }
}

pub proof fn schedule_accumulator_phase_refinement_v1(
    schedule: GeneralGemmScheduleModelV1,
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    lda: nat,
    ldb: nat,
    depth: nat,
)
    ensures
        scheduled_prefix_v1(schedule, a, b, row, column, lda, ldb, depth)
            == dot_prefix_v1(a, b, row, column, lda, ldb, depth),
    decreases depth,
{
    if depth > 0 {
        schedule_accumulator_phase_refinement_v1(
            schedule, a, b, row, column, lda, ldb, (depth - 1) as nat,
        );
    }
}

pub open spec fn epilogue_v1(alpha: real, accumulator: real, beta: real, c: real) -> real {
    alpha * accumulator + beta * c
}

pub proof fn schedule_epilogue_refinement_v1(
    alpha: real,
    accumulator: real,
    beta: real,
    c: real,
)
    ensures epilogue_v1(alpha, accumulator, beta, c)
        == alpha * accumulator + beta * c,
{
}

/// The schedule preserves the declared source operation order over exact
/// values. BF16 decoding, FP32 rounding, instruction selection, and hardware
/// behavior remain premises of the separate machine-refinement boundary.
pub proof fn schedule_numerical_contract_v1(
    schedule: GeneralGemmScheduleModelV1,
    a: Seq<real>,
    b: Seq<real>,
    row: nat,
    column: nat,
    lda: nat,
    ldb: nat,
    k: nat,
    alpha: real,
    beta: real,
    c: real,
)
    ensures
        epilogue_v1(
            alpha,
            scheduled_prefix_v1(schedule, a, b, row, column, lda, ldb, k),
            beta,
            c,
        ) == alpha * dot_prefix_v1(a, b, row, column, lda, ldb, k) + beta * c,
{
    schedule_accumulator_phase_refinement_v1(
        schedule, a, b, row, column, lda, ldb, k,
    );
}

/// The symbolic model proves no claim about an emitted artifact.
pub open spec fn machine_refinement_complete_v1() -> bool { false }

pub proof fn machine_refinement_remains_open_v1()
    ensures !machine_refinement_complete_v1(),
{
}

}
