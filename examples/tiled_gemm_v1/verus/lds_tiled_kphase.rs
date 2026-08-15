use vstd::arithmetic::div_mod::{
    lemma_fundamental_div_mod, lemma_fundamental_div_mod_converse, lemma_mod_bound,
};
use vstd::prelude::*;

#[path = "lds_tiled_slice1.rs"]
mod slice1;

#[path = "tiled_gemm_host_contract.rs"]
mod base;

verus! {

pub open spec fn max_k_phases_v1() -> nat { 4 }

pub open spec fn k_depth_v1(phase_count: nat) -> nat {
    phase_count * 16
}

/// Slice 2 keeps the fixed 16x16 output tile and admits one through four
/// complete K=16 phases. Inputs are mathematical values represented by finite
/// BF16 values after exact widening to F32. IEEE rounding and exceptional
/// values remain outside this model, as in Slice 1.
pub open spec fn bounded_kphase_inputs_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
) -> bool {
    &&& 1 <= phase_count <= max_k_phases_v1()
    &&& a.len() == 16 * k_depth_v1(phase_count)
    &&& b.len() == k_depth_v1(phase_count) * 16
}

pub open spec fn kphase_a_global_index_v1(
    phase_count: nat,
    row: nat,
    depth: nat,
) -> nat {
    row * k_depth_v1(phase_count) + depth
}

pub open spec fn kphase_b_global_index_v1(depth: nat, column: nat) -> nat {
    depth * 16 + column
}

pub open spec fn kphase_depth_v1(phase: nat, offset: nat) -> nat {
    phase * 16 + offset
}

pub open spec fn kphase_a_write_value_v1(
    a: Seq<real>,
    phase_count: nat,
    phase: nat,
    lane: nat,
    component: nat,
) -> real
    recommends
        a.len() == 16 * k_depth_v1(phase_count),
        phase < phase_count,
        lane < 64,
        component < 4,
{
    a[kphase_a_global_index_v1(
        phase_count,
        base::a_register_row_v1(lane),
        kphase_depth_v1(phase, base::a_register_depth_v1(lane, component)),
    ) as int]
}

pub open spec fn kphase_b_write_value_v1(
    b: Seq<real>,
    phase_count: nat,
    phase: nat,
    lane: nat,
    component: nat,
) -> real
    recommends
        b.len() == k_depth_v1(phase_count) * 16,
        phase < phase_count,
        lane < 64,
        component < 4,
{
    b[kphase_b_global_index_v1(
        kphase_depth_v1(phase, base::b_register_depth_v1(lane, component)),
        base::b_register_col_v1(lane),
    ) as int]
}

/// One LDS epoch is used for staged writes and reads. The following epoch is
/// the reuse fence; the next phase cannot begin staging until it completes.
pub open spec fn kphase_write_epoch_v1(phase: nat) -> nat { phase * 2 }
pub open spec fn kphase_read_epoch_v1(phase: nat) -> nat { phase * 2 }
pub open spec fn kphase_reuse_epoch_v1(phase: nat) -> nat { phase * 2 + 1 }

pub open spec fn kphase_stage_event_v1(phase: nat) -> nat { phase * 4 }
pub open spec fn kphase_publish_barrier_event_v1(phase: nat) -> nat { phase * 4 + 1 }
pub open spec fn kphase_read_event_v1(phase: nat) -> nat { phase * 4 + 2 }
pub open spec fn kphase_reuse_barrier_event_v1(phase: nat) -> nat { phase * 4 + 3 }

pub open spec fn kphase_a_read_initialized_v1(
    a: Seq<real>,
    phase_count: nat,
    phase: nat,
    row: nat,
    offset: nat,
) -> bool {
    let lane = slice1::a_writer_lane_v1(row, offset);
    let component = slice1::writer_component_v1(offset);
    &&& phase < phase_count
    &&& lane < 64
    &&& component < 4
    &&& slice1::a_cooperative_write_address_v1(lane, component)
        == slice1::a_lds_address_v1(row, offset)
    &&& kphase_write_epoch_v1(phase) == kphase_read_epoch_v1(phase)
    &&& kphase_a_write_value_v1(a, phase_count, phase, lane, component)
        == a[kphase_a_global_index_v1(
            phase_count, row, kphase_depth_v1(phase, offset),
        ) as int]
}

pub open spec fn kphase_b_read_initialized_v1(
    b: Seq<real>,
    phase_count: nat,
    phase: nat,
    offset: nat,
    column: nat,
) -> bool {
    let lane = slice1::b_writer_lane_v1(offset, column);
    let component = slice1::writer_component_v1(offset);
    &&& phase < phase_count
    &&& lane < 64
    &&& component < 4
    &&& slice1::b_cooperative_write_address_v1(lane, component)
        == slice1::b_lds_address_v1(offset, column)
    &&& kphase_write_epoch_v1(phase) == kphase_read_epoch_v1(phase)
    &&& kphase_b_write_value_v1(b, phase_count, phase, lane, component)
        == b[kphase_b_global_index_v1(
            kphase_depth_v1(phase, offset), column,
        ) as int]
}

pub open spec fn kphase_a_lds_read_value_v1(
    a: Seq<real>,
    phase_count: nat,
    phase: nat,
    row: nat,
    offset: nat,
) -> real
    recommends
        a.len() == 16 * k_depth_v1(phase_count),
        phase < phase_count,
        row < 16,
        offset < 16,
{
    kphase_a_write_value_v1(
        a,
        phase_count,
        phase,
        slice1::a_writer_lane_v1(row, offset),
        slice1::writer_component_v1(offset),
    )
}

pub open spec fn kphase_b_lds_read_value_v1(
    b: Seq<real>,
    phase_count: nat,
    phase: nat,
    offset: nat,
    column: nat,
) -> real
    recommends
        b.len() == k_depth_v1(phase_count) * 16,
        phase < phase_count,
        offset < 16,
        column < 16,
{
    kphase_b_write_value_v1(
        b,
        phase_count,
        phase,
        slice1::b_writer_lane_v1(offset, column),
        slice1::writer_component_v1(offset),
    )
}

proof fn depth_writer_coordinates_kphase_v1(offset: nat)
    requires offset < 16,
    ensures
        offset / 4 < 4,
        offset % 4 < 4,
        offset == (offset / 4) * 4 + offset % 4,
{
    lemma_mod_bound(offset as int, 4);
    lemma_fundamental_div_mod(offset as int, 4);
    assert(offset == 4 * (offset / 4) + offset % 4);
    assert(4 * (offset / 4) == (offset / 4) * 4) by (nonlinear_arith);
    if offset / 4 >= 4 {
        assert((offset / 4) * 4 >= 16) by (nonlinear_arith)
            requires offset / 4 >= 4;
        assert(false);
    }
}

proof fn packed_writer_lane_kphase_v1(axis: nat, offset: nat)
    requires axis < 16, offset < 16,
    ensures
        (offset / 4) * 16 + axis < 64,
        ((offset / 4) * 16 + axis) / 16 == offset / 4,
        ((offset / 4) * 16 + axis) % 16 == axis,
{
    depth_writer_coordinates_kphase_v1(offset);
    lemma_fundamental_div_mod_converse(
        ((offset / 4) * 16 + axis) as int,
        16,
        (offset / 4) as int,
        axis as int,
    );
    assert((offset / 4) * 16 + axis < 64) by (nonlinear_arith)
        requires offset / 4 < 4, axis < 16;
}

/// Every global A/B element loaded by a bounded phase is in range.
pub proof fn bounded_kphase_global_loads_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    phase: nat,
    row: nat,
    offset: nat,
    column: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        phase < phase_count,
        row < 16,
        offset < 16,
        column < 16,
    ensures
        kphase_depth_v1(phase, offset) < k_depth_v1(phase_count),
        kphase_a_global_index_v1(
            phase_count, row, kphase_depth_v1(phase, offset),
        ) < a.len(),
        kphase_b_global_index_v1(
            kphase_depth_v1(phase, offset), column,
        ) < b.len(),
{
    assert(phase + 1 <= phase_count);
    assert(kphase_depth_v1(phase, offset) < (phase + 1) * 16)
        by (nonlinear_arith)
        requires offset < 16;
    assert((phase + 1) * 16 <= phase_count * 16)
        by (nonlinear_arith)
        requires phase + 1 <= phase_count;
    assert((row + 1) * k_depth_v1(phase_count)
        <= 16 * k_depth_v1(phase_count)) by (nonlinear_arith)
        requires row < 16;
    assert(row * k_depth_v1(phase_count) + kphase_depth_v1(phase, offset)
        < (row + 1) * k_depth_v1(phase_count)) by (nonlinear_arith)
        requires kphase_depth_v1(phase, offset) < k_depth_v1(phase_count);
    assert((kphase_depth_v1(phase, offset) + 1) * 16
        <= k_depth_v1(phase_count) * 16) by (nonlinear_arith)
        requires kphase_depth_v1(phase, offset) < k_depth_v1(phase_count);
    assert(kphase_depth_v1(phase, offset) * 16 + column
        < (kphase_depth_v1(phase, offset) + 1) * 16) by (nonlinear_arith)
        requires column < 16;
}

/// Every depth belongs to exactly one admitted 16-wide phase.
pub proof fn bounded_k_phases_partition_depth_v1(
    phase_count: nat,
    depth: nat,
)
    requires 1 <= phase_count <= max_k_phases_v1(), depth < k_depth_v1(phase_count),
    ensures
        depth / 16 < phase_count,
        depth % 16 < 16,
        kphase_depth_v1(depth / 16, depth % 16) == depth,
{
    assert(k_depth_v1(phase_count) > 0) by (nonlinear_arith)
        requires phase_count >= 1;
    assert(k_depth_v1(phase_count) % 16 == 0) by (compute);
    base::k_phases_partition_every_depth_v1(depth, k_depth_v1(phase_count));
    assert(k_depth_v1(phase_count) / 16 == phase_count) by (nonlinear_arith)
        requires phase_count >= 1;
}

/// The canonical cooperative writer supplies A's requested XOR4 LDS slot and
/// the global value for this exact phase and data epoch.
pub proof fn every_kphase_a_read_is_initialized_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    phase: nat,
    row: nat,
    offset: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        phase < phase_count,
        row < 16,
        offset < 16,
    ensures kphase_a_read_initialized_v1(a, phase_count, phase, row, offset),
{
    packed_writer_lane_kphase_v1(row, offset);
    depth_writer_coordinates_kphase_v1(offset);
    bounded_kphase_global_loads_v1(a, b, phase_count, phase, row, offset, 0);
    let lane = slice1::a_writer_lane_v1(row, offset);
    let component = slice1::writer_component_v1(offset);
    assert(base::a_register_row_v1(lane) == row);
    assert(base::a_register_depth_v1(lane, component) == offset);
}

/// The canonical cooperative writer supplies B's requested transposed XOR4
/// LDS slot and the global value for this exact phase and data epoch.
pub proof fn every_kphase_b_read_is_initialized_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    phase: nat,
    offset: nat,
    column: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        phase < phase_count,
        offset < 16,
        column < 16,
    ensures kphase_b_read_initialized_v1(b, phase_count, phase, offset, column),
{
    packed_writer_lane_kphase_v1(column, offset);
    depth_writer_coordinates_kphase_v1(offset);
    bounded_kphase_global_loads_v1(a, b, phase_count, phase, 0, offset, column);
    let lane = slice1::b_writer_lane_v1(offset, column);
    let component = slice1::writer_component_v1(offset);
    assert(base::b_register_depth_v1(lane, component) == offset);
    assert(base::b_register_col_v1(lane) == column);
}

pub open spec fn lane_reaches_kphase_publish_barrier_v1(
    phase_count: nat,
    phase: nat,
    lane: nat,
) -> bool {
    phase < phase_count && lane < 64
}

pub open spec fn lane_reaches_kphase_reuse_barrier_v1(
    phase_count: nat,
    phase: nat,
    lane: nat,
) -> bool {
    phase < phase_count && lane < 64
}

pub open spec fn kphase_barrier_arrivals_v1(
    arrived: Seq<bool>,
    phase_count: nat,
    phase: nat,
    reuse: bool,
) -> bool {
    arrived.len() == 64
        && forall |lane: nat| lane < 64 ==>
            arrived[lane as int] == if reuse {
                lane_reaches_kphase_reuse_barrier_v1(phase_count, phase, lane)
            } else {
                lane_reaches_kphase_publish_barrier_v1(phase_count, phase, lane)
            }
}

/// Both the store-to-read publish barrier and read-to-overwrite reuse barrier
/// are reached by every physical lane in every admitted phase.
pub proof fn kphase_publish_and_reuse_barriers_converge_v1(
    publish_arrived: Seq<bool>,
    reuse_arrived: Seq<bool>,
    phase_count: nat,
    phase: nat,
    lane: nat,
)
    requires
        1 <= phase_count <= max_k_phases_v1(),
        phase < phase_count,
        lane < 64,
        kphase_barrier_arrivals_v1(publish_arrived, phase_count, phase, false),
        kphase_barrier_arrivals_v1(reuse_arrived, phase_count, phase, true),
    ensures
        publish_arrived[lane as int],
        reuse_arrived[lane as int],
        lane_reaches_kphase_publish_barrier_v1(phase_count, phase, lane),
        lane_reaches_kphase_reuse_barrier_v1(phase_count, phase, lane),
{
    assert(forall |physical_lane: nat| physical_lane < 64 ==>
        publish_arrived[physical_lane as int]
            == lane_reaches_kphase_publish_barrier_v1(
                phase_count, phase, physical_lane,
            ));
    assert(forall |physical_lane: nat| physical_lane < 64 ==>
        reuse_arrived[physical_lane as int]
            == lane_reaches_kphase_reuse_barrier_v1(
                phase_count, phase, physical_lane,
            ));
}

/// Any lane's reads of phase `phase` finish before the converged reuse fence,
/// which in turn finishes before any lane can overwrite LDS for `phase + 1`.
pub proof fn no_kphase_overwrite_before_prior_reads_v1(
    phase_count: nat,
    phase: nat,
    reader_lane: nat,
    next_writer_lane: nat,
)
    requires
        1 <= phase_count <= max_k_phases_v1(),
        phase + 1 < phase_count,
        reader_lane < 64,
        next_writer_lane < 64,
    ensures
        kphase_stage_event_v1(phase)
            < kphase_publish_barrier_event_v1(phase),
        kphase_publish_barrier_event_v1(phase)
            < kphase_read_event_v1(phase),
        kphase_read_event_v1(phase)
            < kphase_reuse_barrier_event_v1(phase),
        kphase_reuse_barrier_event_v1(phase)
            < kphase_stage_event_v1(phase + 1),
        kphase_reuse_epoch_v1(phase) < kphase_write_epoch_v1(phase + 1),
        lane_reaches_kphase_reuse_barrier_v1(
            phase_count, phase, reader_lane,
        ),
        lane_reaches_kphase_publish_barrier_v1(
            phase_count, phase + 1, next_writer_lane,
        ),
{
}

pub open spec fn kphase_global_dot_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    end: nat,
) -> real
    recommends
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
        end <= k_depth_v1(phase_count),
    decreases end,
{
    if end == 0 {
        0real
    } else {
        slice1::f32_multiply_add_abstract_v1(
            kphase_global_dot_prefix_v1(
                a, b, phase_count, row, column, (end - 1) as nat,
            ),
            a[kphase_a_global_index_v1(phase_count, row, (end - 1) as nat) as int],
            b[kphase_b_global_index_v1((end - 1) as nat, column) as int],
        )
    }
}

pub open spec fn kphase_lds_accumulate_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    phase: nat,
    offset: nat,
    accumulator: real,
) -> real
    recommends
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
        phase < phase_count,
        offset <= 16,
    decreases offset,
{
    if offset == 0 {
        accumulator
    } else {
        slice1::f32_multiply_add_abstract_v1(
            kphase_lds_accumulate_prefix_v1(
                a,
                b,
                phase_count,
                row,
                column,
                phase,
                (offset - 1) as nat,
                accumulator,
            ),
            kphase_a_lds_read_value_v1(
                a, phase_count, phase, row, (offset - 1) as nat,
            ),
            kphase_b_lds_read_value_v1(
                b, phase_count, phase, (offset - 1) as nat, column,
            ),
        )
    }
}

pub open spec fn kphase_accumulator_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    processed_phases: nat,
) -> real
    recommends
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
        processed_phases <= phase_count,
    decreases processed_phases,
{
    if processed_phases == 0 {
        0real
    } else {
        kphase_lds_accumulate_prefix_v1(
            a,
            b,
            phase_count,
            row,
            column,
            (processed_phases - 1) as nat,
            16,
            kphase_accumulator_v1(
                a,
                b,
                phase_count,
                row,
                column,
                (processed_phases - 1) as nat,
            ),
        )
    }
}

pub open spec fn kphase_accumulator_loop_invariant_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    processed_phases: nat,
    accumulator: real,
) -> bool {
    &&& processed_phases <= phase_count
    &&& accumulator == kphase_global_dot_prefix_v1(
        a, b, phase_count, row, column, processed_phases * 16,
    )
}

proof fn kphase_lds_reads_equal_global_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    phase: nat,
    offset: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
        phase < phase_count,
        offset < 16,
    ensures
        kphase_a_lds_read_value_v1(a, phase_count, phase, row, offset)
            == a[kphase_a_global_index_v1(
                phase_count, row, kphase_depth_v1(phase, offset),
            ) as int],
        kphase_b_lds_read_value_v1(b, phase_count, phase, offset, column)
            == b[kphase_b_global_index_v1(
                kphase_depth_v1(phase, offset), column,
            ) as int],
{
    every_kphase_a_read_is_initialized_v1(
        a, b, phase_count, phase, row, offset,
    );
    every_kphase_b_read_is_initialized_v1(
        a, b, phase_count, phase, offset, column,
    );
}

/// Inner-loop induction: starting from the exact global prefix at this phase's
/// boundary, every LDS-fed multiply-add extends it by one global K element.
pub proof fn kphase_inner_accumulator_invariant_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    phase: nat,
    offset: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
        phase < phase_count,
        offset <= 16,
    ensures
        kphase_lds_accumulate_prefix_v1(
            a,
            b,
            phase_count,
            row,
            column,
            phase,
            offset,
            kphase_global_dot_prefix_v1(
                a, b, phase_count, row, column, phase * 16,
            ),
        ) == kphase_global_dot_prefix_v1(
            a, b, phase_count, row, column, phase * 16 + offset,
        ),
    decreases offset,
{
    if offset > 0 {
        kphase_inner_accumulator_invariant_v1(
            a, b, phase_count, row, column, phase, (offset - 1) as nat,
        );
        kphase_lds_reads_equal_global_v1(
            a, b, phase_count, row, column, phase, (offset - 1) as nat,
        );
        assert(phase * 16 + offset - 1
            == kphase_depth_v1(phase, (offset - 1) as nat));
    }
}

/// The outer phase-loop invariant is initialized by the zero accumulator.
pub proof fn kphase_accumulator_invariant_initial_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
    ensures kphase_accumulator_loop_invariant_v1(
        a, b, phase_count, row, column, 0, 0real,
    ),
{
}

/// One complete LDS phase preserves the accumulator loop invariant. The
/// previous accumulator is an explicit argument and is never reset.
pub proof fn kphase_accumulator_invariant_preserved_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    processed_phases: nat,
    accumulator: real,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
        processed_phases < phase_count,
        kphase_accumulator_loop_invariant_v1(
            a, b, phase_count, row, column, processed_phases, accumulator,
        ),
    ensures kphase_accumulator_loop_invariant_v1(
        a,
        b,
        phase_count,
        row,
        column,
        processed_phases + 1,
        kphase_lds_accumulate_prefix_v1(
            a,
            b,
            phase_count,
            row,
            column,
            processed_phases,
            16,
            accumulator,
        ),
    ),
{
    kphase_inner_accumulator_invariant_v1(
        a, b, phase_count, row, column, processed_phases, 16,
    );
    assert((processed_phases + 1) * 16 == processed_phases * 16 + 16)
        by (nonlinear_arith);
}

proof fn kphase_accumulator_matches_global_prefix_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    row: nat,
    column: nat,
    processed_phases: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        row < 16,
        column < 16,
        processed_phases <= phase_count,
    ensures
        kphase_accumulator_v1(
            a, b, phase_count, row, column, processed_phases,
        ) == kphase_global_dot_prefix_v1(
            a, b, phase_count, row, column, processed_phases * 16,
        ),
    decreases processed_phases,
{
    if processed_phases == 0 {
        kphase_accumulator_invariant_initial_v1(
            a, b, phase_count, row, column,
        );
    } else {
        kphase_accumulator_matches_global_prefix_v1(
            a,
            b,
            phase_count,
            row,
            column,
            (processed_phases - 1) as nat,
        );
        kphase_accumulator_invariant_preserved_v1(
            a,
            b,
            phase_count,
            row,
            column,
            (processed_phases - 1) as nat,
            kphase_accumulator_v1(
                a,
                b,
                phase_count,
                row,
                column,
                (processed_phases - 1) as nat,
            ),
        );
        assert((processed_phases - 1) + 1 == processed_phases);
    }
}

/// Slice 2 retains Slice 1's exhaustive, disjoint ownership of all 256 final
/// C elements; only the K accumulation is extended.
pub proof fn kphase_final_c_stores_are_disjoint_v1(
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
    base::all_unequal_invocations_own_disjoint_global_c_v1(
        0,
        0,
        left_lane,
        left_component,
        0,
        0,
        right_lane,
        right_component,
        16,
        16,
        16,
    );
}

/// For every lane-owned output, all admitted LDS phases compute the complete
/// K-length mathematical matrix product under Slice 1's exact-real arithmetic
/// abstraction.
pub proof fn bounded_kphase_lds_result_is_matrix_product_v1(
    a: Seq<real>,
    b: Seq<real>,
    phase_count: nat,
    lane: nat,
    component: nat,
)
    requires
        bounded_kphase_inputs_v1(a, b, phase_count),
        lane < 64,
        component < 4,
    ensures
        kphase_accumulator_v1(
            a,
            b,
            phase_count,
            base::accumulator_row_v1(lane, component),
            base::accumulator_col_v1(lane),
            phase_count,
        ) == kphase_global_dot_prefix_v1(
            a,
            b,
            phase_count,
            base::accumulator_row_v1(lane, component),
            base::accumulator_col_v1(lane),
            k_depth_v1(phase_count),
        ),
{
    base::accumulator_coordinates_are_bounded_v1(lane, component);
    kphase_accumulator_matches_global_prefix_v1(
        a,
        b,
        phase_count,
        base::accumulator_row_v1(lane, component),
        base::accumulator_col_v1(lane),
        phase_count,
    );
}

} // verus!
